pub mod ash;
mod bindless;
mod bindless_pipeline;

pub use bindless::*;
pub use bindless_pipeline::*;

/// just traits here, only reexport necessary
#[cfg(feature = "primary")]
pub(crate) mod primary {
	pub use crate::generic::platform::*;
}
