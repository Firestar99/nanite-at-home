extern crate core;

/// for macro use
pub use paste;

pub mod reinit;
pub mod vulkan;
pub mod application_config;

/// Call function on drop
struct CallOnDrop<F: FnMut()>(F);

impl<F: FnMut()> Drop for CallOnDrop<F> {
	fn drop(&mut self) {
		self.0()
	}
}
