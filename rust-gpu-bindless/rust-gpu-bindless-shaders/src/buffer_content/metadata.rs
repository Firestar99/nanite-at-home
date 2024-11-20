use crate::frame_in_flight::{FrameInFlight, SeedInFlight};
use rust_gpu_bindless_macros::BufferContent;

/// Metadata about an execution, like the current frame in flight, to be able to safely upgrade weak pointers.
/// Currently unused.
#[derive(Copy, Clone, BufferContent)]
pub struct Metadata;

impl Metadata {
	/// Constructs a fake fif, until this is refactored that Metadata actually forwards the correct fif.
	///
	/// # Safety
	/// as long as TransientDesc discards the fif, we can just make up some garbage
	pub(crate) unsafe fn fake_fif(&self) -> FrameInFlight<'static> {
		unsafe { FrameInFlight::new_unchecked(SeedInFlight::assemble_unchecked(0xDE, 0xA), 0xD) }
	}
}
