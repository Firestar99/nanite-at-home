use core::fmt::{Debug, Formatter};
use vulkano_bindless_macros::BufferContentPlain;

/// a "slice" into a vertex buffer, or rather the start index and len of the slice
#[repr(C)]
#[derive(Copy, Clone, Default, BufferContentPlain)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct MeshletOffset(u32, u32);

impl MeshletOffset {
	#[inline]
	pub fn new(start: usize, len: usize) -> Self {
		assert!(start <= u32::MAX as usize);
		assert!(len <= u32::MAX as usize);
		Self::new_unchecked(start, len)
	}

	#[inline]
	pub fn new_unchecked(start: usize, len: usize) -> Self {
		Self(start as u32, len as u32)
	}

	#[inline]
	pub fn start(&self) -> usize {
		self.0 as usize
	}

	#[inline]
	pub fn len(&self) -> usize {
		self.1 as usize
	}

	#[inline]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}

impl Debug for MeshletOffset {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("MeshletOffset")
			.field("start", &self.start())
			.field("len", &self.len())
			.finish()
	}
}
