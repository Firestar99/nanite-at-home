use crate::meshlet::model::{Meshlet, MeshletModel};
use crate::meshlet::offset::MeshletOffset;
use crate::meshlet::MESHLET_INDICES_BITS;
use core::array;
use core::ops::Index;
use spirv_std::arch::IndexUnchecked;
use vulkano_bindless_macros::DescStruct;
use vulkano_bindless_shaders::descriptor::{BufferSlice, Descriptors, ValidDesc};

#[derive(Copy, Clone, DescStruct)]
#[repr(transparent)]
pub struct CompressedIndices(pub u32);

const INDICES_PER_WORD: usize = 32 / MESHLET_INDICES_BITS as usize;
const INDICES_MASK: u32 = (1 << MESHLET_INDICES_BITS) - 1;

pub struct IndicesReader<S: Source> {
	index_offset: MeshletOffset,
	source: S,
}

impl<'a> IndicesReader<SourceSlice<'a>> {
	pub fn from_slice(slice: &'a [CompressedIndices], meshlet: Meshlet) -> Self {
		Self {
			index_offset: meshlet.index_offset,
			source: SourceSlice(slice),
		}
	}
}

impl<'a> IndicesReader<SourceGpu<'a>> {
	pub fn from_bindless(descriptors: &'a Descriptors, meshlet_model: MeshletModel, meshlet: Meshlet) -> Self {
		Self {
			index_offset: meshlet.index_offset,
			source: SourceGpu(meshlet_model.indices.access(descriptors)),
		}
	}
}

impl<S: Source> IndicesReader<S> {
	fn load_check(&self, triangle: usize) {
		let len = self.len();
		assert!(
			triangle < len,
			"index out of bounds: the len is {len} but the triangle is {triangle}"
		);
	}

	pub fn len(&self) -> usize {
		self.index_offset.len()
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	// gpu
	pub fn load_gpu(&self, triangle: usize) -> [u32; 3] {
		self.load_check(triangle);
		Self::load_gpu_absolute(self.index_offset.start() + triangle, |index| self.source.load(index))
	}

	/// # Safety
	/// index must be in bounds
	pub unsafe fn load_gpu_unchecked(&self, triangle: usize) -> [u32; 3] {
		Self::load_gpu_absolute(self.index_offset.start() + triangle, |index| unsafe {
			self.source.load_unchecked(index)
		})
	}

	#[inline]
	fn load_gpu_absolute(triangle: usize, read_fn: impl Fn(usize) -> CompressedIndices) -> [u32; 3] {
		let mut index = (triangle * 3) / INDICES_PER_WORD;
		let mut rem = (triangle * 3) % INDICES_PER_WORD;
		let mut load = read_fn(index);
		let load0 = (rem, load);
		let mut load_next = || {
			rem += 1;
			if rem == INDICES_PER_WORD {
				rem = 0;
				index += 1;
				load = read_fn(index);
			}
			(rem, load)
		};
		let loads = [load0, load_next(), load_next()];

		loads.map(|(rem, load)| (load.0 >> (rem as u32 * MESHLET_INDICES_BITS)) & INDICES_MASK)
	}

	// cpu
	pub fn load_cpu(&self, triangle: usize) -> [u32; 3] {
		self.load_check(triangle);
		Self::load_cpu_absolute(self.index_offset.start() + triangle, |index| self.source.load(index))
	}

	/// # Safety
	/// index must be in bounds
	pub unsafe fn load_cpu_unchecked(&self, triangle: usize) -> [u32; 3] {
		Self::load_cpu_absolute(self.index_offset.start() + triangle, |index| unsafe {
			self.source.load_unchecked(index)
		})
	}

	fn load_cpu_absolute(triangle: usize, read_fn: impl Fn(usize) -> CompressedIndices) -> [u32; 3] {
		array::from_fn(|i| {
			let i = triangle * 3 + i;
			let index = i / INDICES_PER_WORD;
			let rem = i % INDICES_PER_WORD;
			(read_fn(index).0 >> (rem as u32 * MESHLET_INDICES_BITS)) & INDICES_MASK
		})
	}

	// optimal default
	#[cfg(target_arch = "spirv")]
	pub fn load(&self, triangle: usize) -> [u32; 3] {
		self.load_gpu(triangle)
	}

	/// # Safety
	/// index must be in bounds
	#[cfg(target_arch = "spirv")]
	pub unsafe fn load_unchecked(&self, triangle: usize) -> [u32; 3] {
		self.load_gpu_unchecked(triangle)
	}

	#[cfg(not(target_arch = "spirv"))]
	pub fn load(&self, triangle: usize) -> [u32; 3] {
		self.load_cpu(triangle)
	}

	/// # Safety
	/// index must be in bounds
	#[cfg(not(target_arch = "spirv"))]
	pub unsafe fn load_unchecked(&self, triangle: usize) -> [u32; 3] {
		self.load_cpu_unchecked(triangle)
	}
}

pub trait Source {
	fn load(&self, index: usize) -> CompressedIndices;

	/// # Safety
	/// index must be in bounds
	unsafe fn load_unchecked(&self, index: usize) -> CompressedIndices;
}

pub struct SourceSlice<'a>(&'a [CompressedIndices]);

impl<'a> Source for SourceSlice<'a> {
	fn load(&self, index: usize) -> CompressedIndices {
		*self.0.index(index)
	}

	unsafe fn load_unchecked(&self, index: usize) -> CompressedIndices {
		*self.0.index_unchecked(index)
	}
}

pub struct SourceGpu<'a>(BufferSlice<'a, [CompressedIndices]>);

impl<'a> SourceGpu<'a> {
	pub fn new(descriptors: &'a Descriptors, meshlet_model: MeshletModel) -> Self {
		Self(meshlet_model.indices.access(descriptors))
	}
}

impl<'a> Source for SourceGpu<'a> {
	fn load(&self, index: usize) -> CompressedIndices {
		self.0.load(index)
	}

	unsafe fn load_unchecked(&self, index: usize) -> CompressedIndices {
		self.0.load_unchecked(index)
	}
}

// write
pub fn write_indices_capacity(indices_cnt: usize) -> usize {
	(indices_cnt + INDICES_PER_WORD - 1) / INDICES_PER_WORD
}

pub fn write_indices<Iter>(src: Iter, dst: &mut [CompressedIndices])
where
	Iter: ExactSizeIterator<Item = u32>,
{
	assert_eq!(src.len() % 3, 0, "indices must be multiple of 3");
	let req_len = write_indices_capacity(src.len());
	assert_eq!(
		dst.len(),
		req_len,
		"dst array was length {} instead of required length {}",
		dst.len(),
		req_len
	);

	for (i, s) in src.enumerate() {
		let sm = s & INDICES_MASK;
		assert_eq!(s, sm, "src index {} is too large for {} bits", s, MESHLET_INDICES_BITS);
		let index = i / INDICES_PER_WORD;
		let rem = i % INDICES_PER_WORD;
		dst[index].0 |= sm << (rem as u32 * MESHLET_INDICES_BITS);
	}
}

#[cfg(not(target_arch = "spirv"))]
pub fn write_indices_vec<Iter>(src: Iter) -> Vec<CompressedIndices>
where
	Iter: ExactSizeIterator<Item = u32>,
{
	let mut vec = vec![CompressedIndices(0); write_indices_capacity(src.len())];
	write_indices(src, &mut vec);
	vec
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
	use std::iter::repeat;

	#[test]
	fn write_and_read_verify_quad() {
		write_and_read_verify(&[0, 1, 2, 1, 2, 3]);
	}

	#[test]
	fn write_and_read_verify_limits() {
		let indices: Vec<_> = repeat(0..MESHLET_MAX_VERTICES)
			.flatten()
			.take(3 * MESHLET_MAX_TRIANGLES as usize)
			.collect();
		write_and_read_verify(&indices);
	}

	fn write_and_read_verify(indices: &[u32]) {
		let vec = write_indices_vec(indices.iter().copied());
		let reader = IndicesReader::from_slice(
			&vec,
			Meshlet {
				index_offset: MeshletOffset::new(0, indices.len() / 3),
				vertex_offset: MeshletOffset::default(),
			},
		);
		let read_cpu: Vec<_> = (0..reader.len()).flat_map(|i| reader.load_cpu(i)).collect();
		assert_eq!(indices, read_cpu, "Written and read contents do not agree");
		let read_gpu: Vec<_> = (0..reader.len()).flat_map(|i| reader.load_gpu(i)).collect();
		assert_eq!(indices, read_gpu, "GPU optimized loads do not match written contents");
	}

	#[test]
	#[should_panic(expected = "is too large for")]
	fn vertex_id_oob() {
		write_indices_vec([1u32 << MESHLET_INDICES_BITS, 0, 0].iter().copied());
	}

	#[test]
	fn writing_too_many_indices_is_ok() {
		let indices: Vec<_> = repeat(0..MESHLET_MAX_VERTICES)
			.flatten()
			// note the +1
			.take(3 * (MESHLET_MAX_TRIANGLES as usize + 1))
			.collect();
		write_indices_vec(indices.iter().copied());
	}

	#[test]
	#[should_panic(expected = "but the triangle is")]
	fn reading_indices_oob() {
		let indices = [1, 2, 3];
		let vec = write_indices_vec(indices.iter().copied());
		let reader = IndicesReader::from_slice(
			&vec,
			Meshlet {
				index_offset: MeshletOffset::new(0, indices.len() / 3),
				vertex_offset: MeshletOffset::default(),
			},
		);
		reader.load(2);
	}

	#[test]
	fn index_offset() {
		const INDICES: [&[u32]; 5] = [
			&[1, 2, 3],
			&[4, 5, 6],
			&[7, 8, 9, 10, 11, 12],
			&[42, 36, 12],
			&[23, 63, 16, 38, 26, 48, 22, 34, 60],
		];
		let vec = write_indices_vec(
			INDICES
				.iter()
				.copied()
				.flatten()
				.copied()
				.collect::<Vec<_>>()
				.into_iter(),
		);

		let mut start = 0;
		for indices in INDICES.iter().copied() {
			let triangles = indices.len() / 3;
			let reader = IndicesReader::from_slice(
				&vec,
				Meshlet {
					index_offset: MeshletOffset::new(start, triangles),
					vertex_offset: MeshletOffset::default(),
				},
			);
			for tri in 0..triangles {
				let expect = &indices[tri * 3..tri * 3 + 3];
				unsafe {
					assert_eq!(reader.load_cpu(tri), expect);
					assert_eq!(reader.load_cpu_unchecked(tri), expect);
					assert_eq!(reader.load_gpu(tri), expect);
					assert_eq!(reader.load_gpu_unchecked(tri), expect);
				}
			}
			start += triangles;
		}
	}
}
