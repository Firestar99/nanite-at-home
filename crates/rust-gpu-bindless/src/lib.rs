pub mod generic {
	pub mod backing;
	pub mod descriptor;
	pub mod pipeline;
	pub mod platform;
}

pub use ash::vk::make_api_version;
pub use rust_gpu_bindless_shaders::buffer_content;
pub use rust_gpu_bindless_shaders::shader;
pub use rust_gpu_bindless_shaders::shader_type;
pub use rust_gpu_bindless_shaders::{spirv, spirv_std, Image};

/// primary platform
#[cfg(feature = "primary")]
pub(crate) mod primary {
	/// The primary platform is Ash
	#[cfg(feature = "ash")]
	pub type P = platform::ash::Ash;
	#[cfg(not(any(feature = "ash")))]
	compile_error!("If feature primary is enabled, must select a primary platform feature like \"ash\"");

	pub mod backing {
		pub use crate::generic::backing::primary::*;
	}
	pub mod descriptor {
		pub use crate::generic::descriptor::primary::*;
	}
	pub mod pipeline {
		pub use crate::generic::pipeline::primary::*;
	}
	pub mod platform {
		pub use crate::generic::platform::primary::*;
	}
}
#[cfg(not(feature = "primary"))]
pub(crate) mod primary {
	pub mod descriptor {
		pub use rust_gpu_bindless_shaders::descriptor::*;
	}
}
pub use primary::*;
