extern crate core;

pub use async_global_executor::spawn as spawn;
/// for macro use
pub use paste;

pub mod reinit;
pub mod vulkan;
pub mod application_config;
pub mod space;

pub(crate) mod shader;

/// Call function on drop
pub struct CallOnDrop<F: FnMut()>(pub F);

impl<F: FnMut()> Drop for CallOnDrop<F> {
	fn drop(&mut self) {
		self.0()
	}
}
