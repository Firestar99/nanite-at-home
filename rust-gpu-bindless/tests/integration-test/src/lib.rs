use rust_gpu_bindless::generic::platform::ash::Debuggers;

pub mod buffer_barrier;
pub mod image_copy;
pub mod semaphore;
pub mod shader;
pub mod simple_compute;
pub mod triangle;

/// the global setting on which debugger to use for integration tests
pub fn debugger() -> Debuggers {
	// On Linux RADV gpu assisted validation is segfaulting on graphics pipeline creation
	#[cfg(target_os = "linux")]
	{
		return Debuggers::Validation;
	}
	#[cfg(not(target_os = "linux"))]
	{
		return Debuggers::GpuAssistedValidation;
	}
}
