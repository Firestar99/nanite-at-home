use crate::buffer_content::{Metadata, MetadataCpuInterface};
use crate::descriptor::id::DescriptorId;
use crate::descriptor::transient::TransientDesc;
use crate::descriptor::{AliveDescRef, Desc, DescContent, DescRef, DescStructRef};
use crate::frame_in_flight::FrameInFlight;
use bytemuck_derive::AnyBitPattern;
use static_assertions::const_assert_eq;

#[derive(Copy, Clone)]
pub struct Strong {
	id: DescriptorId,
}

impl DescRef for Strong {}

impl AliveDescRef for Strong {
	#[inline]
	fn id<C: DescContent>(desc: &Desc<Self, C>) -> DescriptorId {
		desc.r.id
	}
}

pub type StrongDesc<C> = Desc<Strong, C>;

impl<C: DescContent> StrongDesc<C> {
	/// Create a new StrongDesc
	///
	/// # Safety
	/// id must be a valid descriptor id that is somehow ensured to stay valid for as long as this StrongDesc exists
	#[inline]
	pub const unsafe fn new(id: DescriptorId) -> Self {
		unsafe { Self::new_inner(Strong { id }) }
	}

	#[inline]
	pub fn to_transient<'a>(&self, frame: FrameInFlight<'a>) -> TransientDesc<'a, C> {
		// Safety: this StrongDesc existing ensures the descriptor will stay alive for this frame
		unsafe { TransientDesc::new(self.id(), frame) }
	}
}

unsafe impl DescStructRef for Strong {
	type TransferDescStruct = TransferStrong;

	unsafe fn desc_write_cpu<C: DescContent>(
		desc: Desc<Self, C>,
		meta: &mut impl MetadataCpuInterface,
	) -> Self::TransferDescStruct {
		meta.visit_strong_descriptor(desc);
		TransferStrong(desc.r.id)
	}

	unsafe fn desc_read<C: DescContent>(from: Self::TransferDescStruct, _meta: Metadata) -> Desc<Self, C> {
		unsafe { StrongDesc::new(from.0) }
	}
}

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct TransferStrong(DescriptorId);
const_assert_eq!(core::mem::size_of::<TransferStrong>(), 4);
