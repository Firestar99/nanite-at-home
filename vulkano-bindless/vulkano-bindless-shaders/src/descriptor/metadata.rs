use crate::frame_in_flight::{FrameInFlight, SeedInFlight};
use bytemuck_derive::AnyBitPattern;
use static_assertions::const_assert;

/// Metadata about an execution, like the current frame in flight, to be able to safely upgrade weak pointers.
/// Currently unused.
#[derive(Copy, Clone, AnyBitPattern)]
pub struct Metadata;

/// Reserve 32 bits of push constant for Metadata
pub const METADATA_MAX_SIZE: usize = 4;
const_assert!(core::mem::size_of::<Metadata>() <= METADATA_MAX_SIZE);

impl Metadata {
	/// Constructs a fake fif, until this is refactored that Metadata actually forwards the correct fif.
	///
	/// # Safety
	/// as long as TransientDesc discards the fif, we can just make up some garbage
	pub(crate) unsafe fn fake_fif(&self) -> FrameInFlight<'static> {
		unsafe { FrameInFlight::new(SeedInFlight::assemble(0xDE, 0xA), 0xD) }
	}
}

/// All bindless push constants are this particular struct, with T being the declared push_param.
///
/// Must not derive `DescStruct`, as to [`DescStruct::from_transfer`] Self you'd need the Metadata, which this struct
/// contains. To break the loop, it just stores Metadata flat and params directly as `T::TransferDescStruct`.
#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct PushConstant<T: bytemuck::AnyBitPattern + Send + Sync> {
	pub t: T,
	pub metadata: Metadata,
}
