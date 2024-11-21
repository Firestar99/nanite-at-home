pub mod ash;
mod bindless;

pub use bindless::*;
use std::error::Error;

/// public interface for a Graphics API. Feel free to use as a base template for other traits.
pub unsafe trait Platform: Sized + Send + Sync + 'static {
	type PlatformCreateInfo: 'static;
	type MemoryAllocation: 'static;
	type Buffer: 'static;
	type TypedBuffer<T: Send + Sync + ?Sized + 'static>: 'static;
	type Image: 'static;
	type ImageView: 'static;
	type Sampler: 'static;
	type AllocationError: 'static + Error;
}
