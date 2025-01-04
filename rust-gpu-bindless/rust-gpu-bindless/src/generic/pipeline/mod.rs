mod access_buffer;
mod access_error;
mod access_image;
mod access_lock;
mod access_type;
mod compute_pipeline;
mod graphics_pipeline;
mod mesh_graphics_pipeline;
mod mut_or_shared;
mod recording;
mod rendering;

pub use access_buffer::*;
pub use access_error::*;
pub use access_image::*;
pub use access_lock::*;
pub use access_type::*;
pub use compute_pipeline::*;
pub use graphics_pipeline::*;
pub use mesh_graphics_pipeline::*;
pub use mut_or_shared::*;
pub use recording::*;
pub use rendering::*;

#[cfg(feature = "primary")]
pub(crate) mod primary {
	pub type MutBufferAccess<'a, T, A> = access_buffer::MutBufferAccess<'a, crate::P, T, A>;
	pub type MutImageAccess<'a, T, A> = access_image::MutImageAccess<'a, crate::P, T, A>;
	pub type BindlessComputePipeline<T> = compute_pipeline::BindlessComputePipeline<crate::P, T>;
	pub type BindlessGraphicsPipeline<T> = graphics_pipeline::BindlessGraphicsPipeline<crate::P, T>;
	pub type BindlessMeshGraphicsPipeline<T> = mesh_graphics_pipeline::BindlessMeshGraphicsPipeline<crate::P, T>;
	pub type RecordingError = recording::RecordingError<crate::P>;
	pub type Recording<'a> = recording::Recording<'a, crate::P>;
	pub type Rendering<'a, 'b> = rendering::Rendering<'a, 'b, crate::P>;
	pub type RenderingAttachment<'a, 'b, A> = rendering::RenderingAttachment<'a, 'b, crate::P, A>;

	pub use crate::generic::pipeline::*;
}
