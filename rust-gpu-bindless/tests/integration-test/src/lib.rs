use rust_gpu_bindless_core::platform::ash::Debuggers;

pub mod buffer_barrier;
pub mod image_copy;
pub mod semaphore;
pub mod shader;
pub mod simple_compute;
pub mod triangle;

/// the global setting on which debugger to use for integration tests
pub fn debugger() -> Debuggers {
	// Validation layer does not yet support timelime semaphores properly, leading to many false positives.
	// On Linux RADV gpu assisted validation even segfaulting on graphics pipeline creation.
	Debuggers::Validation
}
