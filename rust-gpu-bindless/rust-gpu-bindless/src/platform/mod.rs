pub mod ash;
mod bindless;
mod bindless_pipeline;

pub use bindless::*;
pub use bindless_pipeline::*;
use std::error::Error;

/// public interface for a Graphics API. Feel free to use as a base template for other traits.
pub unsafe trait Platform: Sized + Send + Sync + 'static {
	type PlatformCreateInfo: 'static;
	type MemoryAllocation: 'static;
	type Buffer: 'static;
	type TypedBuffer<T: Send + Sync + ?Sized>: 'static;
	type Image: 'static;
	type ImageView: 'static;
	type Sampler: 'static;
	type AllocationError: 'static + Error;
}
