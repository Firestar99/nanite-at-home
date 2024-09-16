use crate::buffer_content::Metadata;
use crate::descriptor::transient::TransientDesc;
use crate::descriptor::{Desc, DescContent, DescRef};
use bytemuck_derive::{Pod, Zeroable};
use core::mem;
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

/// The raw unsafe descriptor identifier to locate a resource. Internally it's a bit packed u32 containing the
/// [`DescriptorType`], [`DescriptorIndex`] and version. All other descriptors use `DescriptorId` internally.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Zeroable, Pod, BufferContent)]
pub struct DescriptorId(u32);
const_assert_eq!(mem::size_of::<DescriptorId>(), 4);

impl DescriptorId {
	pub unsafe fn new(desc_type: DescriptorType, index: DescriptorIndex, version: DescriptorVersion) -> Self {
		let mut value = 0;
		value |= (desc_type.0 & ID_TYPE_MASK) << ID_TYPE_SHIFT;
		value |= (index.0 & ID_INDEX_MASK) << ID_INDEX_SHIFT;
		value |= (version.0 & ID_VERSION_MASK) << ID_VERSION_SHIFT;
		Self(value)
	}

	pub const fn desc_type(&self) -> DescriptorType {
		DescriptorType((self.0 >> ID_TYPE_SHIFT) & ID_TYPE_MASK)
	}

	pub const fn index(&self) -> DescriptorIndex {
		DescriptorIndex((self.0 >> ID_INDEX_SHIFT) & ID_INDEX_MASK)
	}

	pub const fn version(&self) -> DescriptorVersion {
		DescriptorVersion((self.0 >> ID_VERSION_SHIFT) & ID_VERSION_MASK)
	}
}

impl DescRef for DescriptorId {}

/// The descriptor table type of [`DescriptorId`].
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, BufferContent)]
pub struct DescriptorType(u32);
const_assert_eq!(mem::size_of::<DescriptorType>(), 4);

impl DescriptorType {
	pub unsafe fn new(type_id: u32) -> Option<Self> {
		if type_id == type_id & ID_TYPE_MASK {
			Some(Self::new_unchecked(type_id))
		} else {
			None
		}
	}

	pub unsafe fn new_unchecked(type_id: u32) -> Self {
		Self(type_id)
	}

	pub const fn to_u32(&self) -> u32 {
		self.0
	}

	pub const fn to_usize(&self) -> usize {
		self.0 as usize
	}
}

/// The index of [`DescriptorId`].
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, BufferContent)]
pub struct DescriptorIndex(u32);
const_assert_eq!(mem::size_of::<DescriptorIndex>(), 4);

impl DescriptorIndex {
	pub unsafe fn new(index: u32) -> Option<Self> {
		if index == index & ID_INDEX_MASK {
			Some(Self::new_unchecked(index))
		} else {
			None
		}
	}

	pub unsafe fn new_unchecked(index: u32) -> Self {
		Self(index)
	}

	pub const fn to_u32(&self) -> u32 {
		self.0
	}

	pub const fn to_usize(&self) -> usize {
		self.0 as usize
	}
}

/// The version of [`DescriptorId`].
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, BufferContent)]
pub struct DescriptorVersion(u32);
const_assert_eq!(mem::size_of::<DescriptorVersion>(), 4);

impl DescriptorVersion {
	pub unsafe fn new(version: u32) -> Option<Self> {
		if version == version & ID_VERSION_MASK {
			Some(Self::new_unchecked(version))
		} else {
			None
		}
	}

	pub unsafe fn new_unchecked(version: u32) -> Self {
		Self(version)
	}

	pub const fn to_u32(&self) -> u32 {
		self.0
	}
}

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
