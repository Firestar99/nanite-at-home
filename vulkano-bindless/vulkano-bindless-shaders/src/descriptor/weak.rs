use crate::descriptor::transient::TransientDesc;
use crate::descriptor::{Desc, DescContent, DescRef, Metadata};
use core::mem;
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::BufferContent;

#[derive(Copy, Clone, BufferContent)]
pub struct Weak {
	id: u32,
	version: u32,
}
const_assert_eq!(mem::size_of::<Weak>(), 8);

impl DescRef for Weak {}

pub type WeakDesc<C> = Desc<Weak, C>;

impl<C: DescContent> WeakDesc<C> {
	/// Creates a new WeakDesc
	///
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to
	#[inline]
	pub const unsafe fn new(id: u32, version: u32) -> WeakDesc<C> {
		unsafe { Self::new_inner(Weak { id, version }) }
	}

	#[inline]
	pub const fn id(&self) -> u32 {
		self.r.id
	}

	#[inline]
	pub const fn version(&self) -> u32 {
		self.r.version
	}

	/// Upgrades a WeakDesc to a TransientDesc that is valid for the current frame in flight, assuming the descriptor
	/// pointed to is still valid.
	///
	/// # Safety
	/// This unsafe variant assumes the descriptor is still alive, rather than checking whether it actually is.
	#[inline]
	pub unsafe fn upgrade_unchecked<'a>(&self, meta: Metadata) -> TransientDesc<'a, C> {
		unsafe { TransientDesc::new(self.r.id, meta.fake_fif()) }
	}
}
