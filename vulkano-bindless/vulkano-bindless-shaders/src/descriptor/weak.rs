use crate::buffer_content::Metadata;
use crate::descriptor::id::DescriptorId;
use crate::descriptor::transient::TransientDesc;
use crate::descriptor::{Desc, DescContent, DescRef};
use vulkano_bindless_macros::{assert_transfer_size, BufferContent};

#[derive(Copy, Clone, BufferContent)]
pub struct Weak {
	id: DescriptorId,
}
assert_transfer_size!(Weak, 4);

impl DescRef for Weak {}

pub type WeakDesc<C> = Desc<Weak, C>;

impl<C: DescContent> WeakDesc<C> {
	/// Creates a new WeakDesc
	///
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to
	#[inline]
	pub const unsafe fn new(id: DescriptorId) -> WeakDesc<C> {
		unsafe { Self::new_inner(Weak { id }) }
	}

	#[inline]
	pub const fn id(&self) -> DescriptorId {
		self.r.id
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
