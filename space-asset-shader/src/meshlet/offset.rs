use crate::meshlet::{MESHLET_INDICES_BITS, MESHLET_TRIANGLES_BITS};
use core::ops::{Index, Range};
use static_assertions::const_assert;
use vulkano_bindless_macros::BufferContent;

const START_BITS: u32 = 24;
const START_LIMIT: u32 = 1 << START_BITS;
const START_MASK: u32 = START_LIMIT - 1;
const LEN_BITS: u32 = 8;
const LEN_LIMIT: u32 = 1 << LEN_BITS;
const LEN_MASK: u32 = LEN_LIMIT - 1;
const LEN_SHIFT: u32 = START_BITS;

const_assert!(LEN_BITS >= MESHLET_INDICES_BITS);
const_assert!(LEN_BITS >= MESHLET_TRIANGLES_BITS);

/// a "slice" into a vertex buffer, or rather the start index and len of the slice
#[derive(Copy, Clone, BufferContent, Default)]
#[repr(transparent)]
pub struct MeshletOffset(u32);

impl MeshletOffset {
	#[inline]
	pub fn new(start: usize, len: usize) -> Self {
		assert!(start < START_LIMIT as usize);
		assert!(len < LEN_LIMIT as usize);
		Self::new_unchecked(start, len)
	}

	#[inline]
	pub fn new_unchecked(start: usize, len: usize) -> Self {
		Self(start as u32 & START_MASK | (len as u32 & LEN_MASK) << LEN_SHIFT)
	}

	#[inline]
	pub fn start(&self) -> usize {
		(self.0 & START_MASK) as usize
	}

	#[inline]
	pub fn len(&self) -> usize {
		(self.0 >> LEN_SHIFT) as usize
	}

	#[inline]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	#[inline]
	pub fn slice<'a, R: Index<Range<usize>>>(&self, slice: &'a R) -> &'a R::Output {
		slice.index(self.start()..(self.start() + self.len()))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn meshlet_index() {
		for start_sh in 0..START_BITS {
			let start = (1 << start_sh) - 1;
			for len_sh in 0..LEN_BITS {
				let len = (1 << len_sh) - 1;
				let slice = MeshletOffset::new(start, len);
				assert_eq!(slice.start(), start);
				assert_eq!(slice.len(), len);
			}
		}
	}

	#[test]
	#[should_panic(expected = "start < START_")]
	fn meshlet_index_oob_start() {
		MeshletOffset::new(1 << START_BITS, 1);
	}

	#[test]
	#[should_panic(expected = "len < LEN_")]
	fn meshlet_index_oob_len() {
		MeshletOffset::new(0, 1 << LEN_BITS);
	}

	#[test]
	fn meshlet_default() {
		let offset = MeshletOffset::default();
		assert_eq!(offset.start(), 0);
		assert_eq!(offset.len(), 0);
	}
}
