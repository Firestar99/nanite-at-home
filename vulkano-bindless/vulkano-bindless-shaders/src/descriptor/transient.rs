use crate::buffer_content::{Metadata, MetadataCpuInterface};
use crate::descriptor::id::DescriptorId;
use crate::descriptor::{AliveDescRef, Desc, DescContent, DescRef, DescStructRef};
use crate::frame_in_flight::FrameInFlight;
use bytemuck_derive::AnyBitPattern;
use core::marker::PhantomData;
use core::mem;
use static_assertions::const_assert_eq;

#[derive(Copy, Clone)]
pub struct Transient<'a> {
	id: DescriptorId,
	_phantom: PhantomData<&'a ()>,
}
const_assert_eq!(mem::size_of::<Transient>(), 4);

impl<'a> DescRef for Transient<'a> {}

impl<'a> AliveDescRef for Transient<'a> {
	#[inline]
	fn id<C: DescContent>(desc: &Desc<Self, C>) -> DescriptorId {
		desc.r.id
	}
}

pub type TransientDesc<'a, C> = Desc<Transient<'a>, C>;

impl<'a, C: DescContent> TransientDesc<'a, C> {
	/// Create a new TransientDesc
	///
	/// # Safety
	/// * The C generic must match the content that the [`DescRef`] points to.
	/// * id must be a valid descriptor id that stays valid for the remainder of the frame `'a`.
	#[inline]
	pub const unsafe fn new(id: DescriptorId, frame_in_flight: FrameInFlight<'a>) -> Self {
		// We just need the lifetime of the frame, no need to actually store the value.
		// Apart from maybe future validation?
		// If this value is ever used, weak's upgrade_unchecked() needs to be adjusted accordingly!
		let _ = frame_in_flight;
		unsafe {
			Self::new_inner(Transient {
				id,
				_phantom: PhantomData {},
			})
		}
	}
}

unsafe impl<'a> DescStructRef for Transient<'a> {
	type TransferDescStruct = TransferTransient;

	unsafe fn desc_write_cpu<C: DescContent>(
		desc: Desc<Self, C>,
		_meta: &mut impl MetadataCpuInterface,
	) -> Self::TransferDescStruct {
		Self::TransferDescStruct { id: desc.r.id }
	}

	unsafe fn desc_read<C: DescContent>(from: Self::TransferDescStruct, _meta: Metadata) -> Desc<Self, C> {
		unsafe { TransientDesc::new(from.id, _meta.fake_fif()) }
	}
}

#[repr(transparent)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct TransferTransient {
	id: DescriptorId,
}
const_assert_eq!(mem::size_of::<TransferTransient>(), 4);
