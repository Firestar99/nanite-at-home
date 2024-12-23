pub mod ash;
mod bindless;
mod bindless_pipeline;

pub use bindless::*;
pub use bindless_pipeline::*;

use crate::backing::table::SlotAllocationError;
use crate::pipeline::access_error::AccessError;
use std::error::Error;

/// public interface for a Graphics API. Feel free to use as a base template for other traits.
pub unsafe trait Platform: Sized + Send + Sync + 'static {
	type PlatformCreateInfo: 'static;
	type MemoryAllocation: 'static + Send + Sync;
	type Buffer: 'static + Send + Sync;
	type TypedBuffer<T: Send + Sync + ?Sized>: 'static + Send + Sync;
	type Image: 'static + Send + Sync;
	type ImageView: 'static + Send + Sync;
	type Sampler: 'static + Send + Sync;
	type AllocationError: 'static + Error + Send + Sync + From<SlotAllocationError> + From<AccessError>;
}
