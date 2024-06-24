use crate::meshlet::mesh::MeshletData;
use crate::meshlet::MESHLET_INDICES_BITS;
use core::array;
use glam::UVec3;
use vulkano_bindless_macros::BufferContent;

#[derive(Copy, Clone, BufferContent)]
#[repr(transparent)]
pub struct CompressedIndices(pub u32);

const INDICES_PER_WORD: usize = 32 / MESHLET_INDICES_BITS as usize;
const INDICES_MASK: u32 = (1 << MESHLET_INDICES_BITS) - 1;

// `t: T` is passed though to the function and its mainly used so BufferDescriptors can be passed though, as you can't
// put them in a closure without rust-gpu compiling them as illegal function pointers
pub fn triangle_indices_load_gpu<T>(
	meshlet: impl AsRef<MeshletData>,
	t: &T,
	triangle: usize,
	read_fn: impl Fn(&T, usize) -> CompressedIndices,
) -> UVec3 {
	let abs_triangle = meshlet.as_ref().triangle_indices_offset.start() + triangle;
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
	let abs_triangle = meshlet.as_ref().triangle_indices_offset.start() + triangle;
	UVec3::from_array(array::from_fn(|i| {
		let i = abs_triangle * 3 + i;
		let index = i / INDICES_PER_WORD;
		let rem = i % INDICES_PER_WORD;
		(read_fn(t, index).0 >> (rem as u32 * MESHLET_INDICES_BITS)) & INDICES_MASK
	}))
}

// optimal default
#[cfg(target_arch = "spirv")]
pub fn triangle_indices_load<T>(
	meshlet: impl AsRef<MeshletData>,
	t: &T,
	triangle: usize,
	read_fn: impl Fn(&T, usize) -> CompressedIndices,
) -> UVec3 {
	triangle_indices_load_gpu(meshlet, t, triangle, read_fn)
}

#[cfg(not(target_arch = "spirv"))]
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
	(indices_cnt + INDICES_PER_WORD - 1) / INDICES_PER_WORD
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

#[cfg(not(target_arch = "spirv"))]
pub fn triangle_indices_write_vec<Iter>(src: Iter) -> Vec<CompressedIndices>
where
	Iter: ExactSizeIterator<Item = u32>,
{
	let mut vec = vec![CompressedIndices(0); triangle_indices_write_capacity(src.len())];
	triangle_indices_write(src, &mut vec);
	vec
}

#[cfg(test)]
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
			triangle_indices_offset: MeshletOffset::new(0, indices.len() / 3),
			vertex_offset: MeshletOffset::default(),
		};
		let read_cpu: Vec<_> = (0..meshlet.triangle_indices_offset.len())
			.flat_map(|i| triangle_indices_load_cpu(&meshlet, &(), i, |_, i| *vec.get(i).unwrap()).to_array())
			.collect();
		assert_eq!(indices, read_cpu, "Written and read contents do not agree");
		let read_gpu: Vec<_> = (0..meshlet.triangle_indices_offset.len())
			.flat_map(|i| triangle_indices_load_gpu(&meshlet, &(), i, |_, i| *vec.get(i).unwrap()).to_array())
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
				triangle_indices_offset: MeshletOffset::new(start, triangles),
				vertex_offset: MeshletOffset::default(),
			};
			for tri in 0..triangles {
				let expect = &indices[tri * 3..tri * 3 + 3];
				assert_eq!(
					triangle_indices_load_cpu(&meshlet, &(), tri, |_, i| *vec.get(i).unwrap()).to_array(),
					expect
				);
				assert_eq!(
					triangle_indices_load_gpu(&meshlet, &(), tri, |_, i| *vec.get(i).unwrap()).to_array(),
					expect
				);
			}
			start += triangles;
		}
	}
}
