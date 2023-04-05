extern crate core;

/// for macro use
pub use paste;
pub use async_global_executor::spawn as spawn;

pub mod reinit;
pub mod vulkan;
pub mod application_config;
pub mod space;

/// Call function on drop
pub struct CallOnDrop<F: FnMut()>(pub F);

impl<F: FnMut()> Drop for CallOnDrop<F> {
	fn drop(&mut self) {
		self.0()
	}
}
