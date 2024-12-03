use crate::meshlet::mesh::MeshletData;
use crate::meshlet::MESHLET_INDICES_BITS;
use core::fmt::Debug;
use core::fmt::Formatter;
use glam::UVec3;
use rust_gpu_bindless_macros::BufferStructPlain;

#[repr(transparent)]
#[derive(Copy, Clone, Default, BufferStructPlain)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct CompressedIndices(pub u32);

impl CompressedIndices {
	pub fn to_values(&self) -> [u32; INDICES_PER_WORD] {
		let f = |i| (self.0 >> (i * MESHLET_INDICES_BITS as usize)) & INDICES_MASK;
		[f(0), f(1), f(2), f(3), f(4)]
	}

	#[allow(clippy::needless_range_loop)]
	pub fn from_values(values: [u32; INDICES_PER_WORD]) -> Self {
		let mut out = 0;
		for i in 0..INDICES_PER_WORD {
			out |= (values[i] & INDICES_MASK) << (i * MESHLET_INDICES_BITS as usize);
		}
		Self(out)
	}
}

impl Debug for CompressedIndices {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		for i in self.to_values() {
			write!(f, "{:3}", i)?;
		}
		Ok(())
	}
}

pub const INDICES_PER_WORD: usize = 32 / MESHLET_INDICES_BITS as usize;
pub const INDICES_MASK: u32 = (1 << MESHLET_INDICES_BITS) - 1;

// `t: T` is passed though to the function and its mainly used so BufferDescriptors can be passed though, as you can't
// put them in a closure without rust-gpu compiling them as illegal function pointers
pub fn triangle_indices_load_gpu<T>(
	meshlet: impl AsRef<MeshletData>,
	t: &T,
	triangle: usize,
	read_fn: impl Fn(&T, usize) -> CompressedIndices,
) -> UVec3 {
	let abs_triangle = meshlet.as_ref().triangle_offset.start() + triangle;
	let mut index = (abs_triangle * 3) / INDICES_PER_WORD;
	let mut rem = (abs_triangle * 3) % INDICES_PER_WORD;
	let mut load = read_fn(t, index);
	let load0 = (rem, load);
	let mut load_next = || {
		rem += 1;
		if rem == INDICES_PER_WORD {
			rem = 0;
			index += 1;
			load = read_fn(t, index);
		}
		(rem, load)
	};
	let loads = [load0, load_next(), load_next()];

	let f = |(rem, load): (usize, CompressedIndices)| (load.0 >> (rem as u32 * MESHLET_INDICES_BITS)) & INDICES_MASK;
	UVec3::new(f(loads[0]), f(loads[1]), f(loads[2]))
}

pub fn triangle_indices_load_cpu<T>(
	meshlet: impl AsRef<MeshletData>,
	t: &T,
	triangle: usize,
	read_fn: impl Fn(&T, usize) -> CompressedIndices,
) -> UVec3 {
	let abs_triangle = meshlet.as_ref().triangle_offset.start() + triangle;
	let f = |i| {
		let i = abs_triangle * 3 + i;
		let index = i / INDICES_PER_WORD;
		let rem = i % INDICES_PER_WORD;
		(read_fn(t, index).0 >> (rem as u32 * MESHLET_INDICES_BITS)) & INDICES_MASK
	};
	UVec3::from_array([f(0), f(1), f(2)])
}

// optimal default
#[cfg(target_arch = "spirv")]
#[inline]
pub fn triangle_indices_load<T>(
	meshlet: impl AsRef<MeshletData>,
	t: &T,
	triangle: usize,
	read_fn: impl Fn(&T, usize) -> CompressedIndices,
) -> UVec3 {
	triangle_indices_load_gpu(meshlet, t, triangle, read_fn)
}

#[cfg(not(target_arch = "spirv"))]
#[inline]
pub fn triangle_indices_load<T>(
	meshlet: impl AsRef<MeshletData>,
	t: &T,
	triangle: usize,
	read_fn: impl Fn(&T, usize) -> CompressedIndices,
) -> UVec3 {
	triangle_indices_load_cpu(meshlet, t, triangle, read_fn)
}

// write
pub fn triangle_indices_write_capacity(indices_cnt: usize) -> usize {
	let required_words = (indices_cnt + INDICES_PER_WORD - 1) / INDICES_PER_WORD;
	let padded_next_mul_of_3 = (required_words + 3 - 1) / 3 * 3;
	padded_next_mul_of_3
}

pub fn triangle_indices_write<Iter>(src: Iter, dst: &mut [CompressedIndices])
where
	Iter: ExactSizeIterator<Item = u32>,
{
	assert_eq!(src.len() % 3, 0, "indices must be multiple of 3");
	let req_len = triangle_indices_write_capacity(src.len());
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

#[cfg(feature = "disk")]
pub fn triangle_indices_write_vec<Iter>(src: Iter) -> std::vec::Vec<CompressedIndices>
where
	Iter: ExactSizeIterator<Item = u32>,
{
	let mut vec = std::vec![CompressedIndices(0); triangle_indices_write_capacity(src.len())];
	triangle_indices_write(src, &mut vec);
	vec
}

#[cfg(test)]
#[cfg(feature = "disk")]
mod tests {
	use super::*;
	use crate::meshlet::offset::MeshletOffset;
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
		let vec = triangle_indices_write_vec(indices.iter().copied());
		let meshlet = MeshletData {
			triangle_offset: MeshletOffset::new(0, indices.len() / 3),
			draw_vertex_offset: MeshletOffset::default(),
		};
		let read_cpu: Vec<_> = (0..meshlet.triangle_offset.len())
			.flat_map(|i| triangle_indices_load_cpu(meshlet, &(), i, |_, i| *vec.get(i).unwrap()).to_array())
			.collect();
		assert_eq!(indices, read_cpu, "Written and read contents do not agree");
		let read_gpu: Vec<_> = (0..meshlet.triangle_offset.len())
			.flat_map(|i| triangle_indices_load_gpu(meshlet, &(), i, |_, i| *vec.get(i).unwrap()).to_array())
			.collect();
		assert_eq!(indices, read_gpu, "GPU optimized loads do not match written contents");
	}

	#[test]
	#[should_panic(expected = "is too large for")]
	fn vertex_id_oob() {
		triangle_indices_write_vec([1u32 << MESHLET_INDICES_BITS, 0, 0].iter().copied());
	}

	#[test]
	fn writing_too_many_indices_is_ok() {
		let indices: Vec<_> = repeat(0..MESHLET_MAX_VERTICES)
			.flatten()
			// note the +1
			.take(3 * (MESHLET_MAX_TRIANGLES as usize + 1))
			.collect();
		triangle_indices_write_vec(indices.iter().copied());
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
		let vec = triangle_indices_write_vec(
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
			let meshlet = MeshletData {
				triangle_offset: MeshletOffset::new(start, triangles),
				draw_vertex_offset: MeshletOffset::default(),
			};
			for tri in 0..triangles {
				let expect = &indices[tri * 3..tri * 3 + 3];
				assert_eq!(
					triangle_indices_load_cpu(meshlet, &(), tri, |_, i| *vec.get(i).unwrap()).to_array(),
					expect
				);
				assert_eq!(
					triangle_indices_load_gpu(meshlet, &(), tri, |_, i| *vec.get(i).unwrap()).to_array(),
					expect
				);
			}
			start += triangles;
		}
	}

	#[test]
	#[allow(clippy::needless_range_loop)]
	fn from_to_value() {
		let test_array = |mul: u32, off: u32| {
			let mut array = [0; INDICES_PER_WORD];
			for i in 0..INDICES_PER_WORD {
				array[i] = (i as u32 * mul + off) & INDICES_MASK;
			}
			assert_eq!(CompressedIndices::from_values(array).to_values(), array);
		};
		test_array(1, 0);
		test_array(0, 42);
		test_array(123, 90);
	}
}
