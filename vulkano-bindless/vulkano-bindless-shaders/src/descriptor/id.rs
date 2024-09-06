use crate::buffer_content::Metadata;
use crate::descriptor::transient::TransientDesc;
use crate::descriptor::{Desc, DescContent, DescContentType, DescRef};
use bytemuck_derive::{Pod, Zeroable};
use core::mem;
use num_traits::{FromPrimitive, ToPrimitive};
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::BufferContent;

pub const ID_INDEX_BITS: u32 = 18;
pub const ID_TYPE_BITS: u32 = 2;
pub const ID_VERSION_BITS: u32 = 12;

const ID_INDEX_MASK: u32 = (1 << ID_INDEX_BITS) - 1;
const ID_TYPE_MASK: u32 = (1 << ID_TYPE_BITS) - 1;
const ID_VERSION_MASK: u32 = (1 << ID_VERSION_BITS) - 1;

const ID_INDEX_SHIFT: u32 = 0;
const ID_TYPE_SHIFT: u32 = ID_INDEX_BITS;
const ID_VERSION_SHIFT: u32 = ID_INDEX_BITS + ID_TYPE_BITS;

// uses all 32 bits
const_assert_eq!(ID_INDEX_BITS + ID_TYPE_BITS + ID_VERSION_BITS, 32);
// masks use entire 32 bit range
const_assert_eq!(
	ID_INDEX_MASK << ID_INDEX_SHIFT | ID_TYPE_MASK << ID_TYPE_SHIFT | ID_VERSION_MASK << ID_VERSION_SHIFT,
	!0
);
// masks do not overlap
const_assert_eq!(ID_INDEX_MASK << ID_INDEX_SHIFT & ID_TYPE_MASK << ID_TYPE_SHIFT, 0);
const_assert_eq!(ID_INDEX_MASK << ID_INDEX_SHIFT & ID_VERSION_MASK << ID_VERSION_SHIFT, 0);
const_assert_eq!(ID_TYPE_MASK << ID_TYPE_SHIFT & ID_VERSION_MASK << ID_VERSION_SHIFT, 0);

/// An [`UnsafeDesc`] that does not verify that the resource is actually alive or not, and thus is fully unsafe to use.
/// The basis of most other descriptor types.
#[repr(transparent)]
#[derive(Copy, Clone, Zeroable, Pod, BufferContent)]
pub struct DescriptorId(u32);
const_assert_eq!(mem::size_of::<DescriptorId>(), 4);

impl DescriptorId {
	pub unsafe fn new(content_type: DescContentType, index: u32, version: u32) -> Self {
		// cannot fail, as ensured by const_assert on DescContentType
		let content_type = content_type.to_u32().unwrap_or_default();
		let mut value = 0;
		value |= (content_type & ID_TYPE_MASK) << ID_TYPE_SHIFT;
		value |= (index & ID_INDEX_MASK) << ID_INDEX_SHIFT;
		value |= (version & ID_VERSION_MASK) << ID_VERSION_SHIFT;
		Self(value)
	}

	pub fn content_type(&self) -> DescContentType {
		match DescContentType::from_u32((self.0 >> ID_TYPE_SHIFT) & ID_TYPE_MASK) {
			Some(e) => e,
			None => {
				// FIXME unreachable on spirv?
				// // I wish spirv cound panic better, but this should be unreachable anyways
				// #[cfg(not(target_arch = "spirv"))]
				unreachable!("Invalid ContentType bits");
				// #[cfg(target_arch = "spirv")]
				// DescContentType::Buffer
			}
		}
	}

	pub const fn index(&self) -> u32 {
		(self.0 >> ID_INDEX_SHIFT) & ID_INDEX_MASK
	}

	pub const fn version(&self) -> u32 {
		(self.0 >> ID_VERSION_SHIFT) & ID_VERSION_MASK
	}
}

impl DescRef for DescriptorId {}

pub type UnsafeDesc<C> = Desc<DescriptorId, C>;

impl<C: DescContent> UnsafeDesc<C> {
	/// Creates a new UnsafeDesc
	///
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to
	#[inline]
	pub const unsafe fn new(id: DescriptorId) -> UnsafeDesc<C> {
		unsafe { Self::new_inner(id) }
	}

	#[inline]
	pub const fn id(&self) -> DescriptorId {
		self.r
	}

	#[inline]
	pub unsafe fn to_transient_unchecked<'a>(&self, meta: Metadata) -> TransientDesc<'a, C> {
		unsafe { TransientDesc::new(self.r, meta.fake_fif()) }
	}
}
