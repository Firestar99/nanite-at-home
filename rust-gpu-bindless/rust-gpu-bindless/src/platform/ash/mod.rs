mod access_type;
/// Extensions to ash directly. Usually functions that would return a `Vec<_>`, but are often called with only one
/// element, optimized to not allocate a `Vec`.
mod ash_ext;
/// main BindlessPlatform trait impl
mod bindless;
/// BindlessPipelinePlatform impl
mod bindless_pipeline;
/// Conversion of types from bindless to ash
mod convert;
/// Execution tracking with timeline semaphores
mod executing;
/// Image format enum tables
mod image_format;
/// Simple init function to create a device with a single graphics queue
mod init;
/// CommandBuffer recording
mod recording;
/// CommandBuffer recording of rendering cmds
mod rendering;
/// Extending tables with ash specific functionality, usually alloc methods taking ash CreateInfos
mod table_ext;

pub use access_type::*;
pub use ash_ext::*;
pub use bindless::*;
pub use convert::*;
pub use executing::*;
pub use init::*;
pub use recording::*;

pub struct RunOnDrop<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> RunOnDrop<F> {
	pub fn new(f: F) -> Self {
		Self(Some(f))
	}

	pub fn take(mut self) -> F {
		self.0.take().unwrap()
	}
}

impl<F: FnOnce()> Drop for RunOnDrop<F> {
	fn drop(&mut self) {
		if let Some(f) = self.0.take() {
			f()
		}
	}
}
