pub mod access_buffer;
pub mod access_error;
pub mod access_image;
pub mod access_lock;
pub mod access_type;
pub mod compute_pipeline;
pub mod graphics_pipeline;
pub mod mesh_graphics_pipeline;
pub mod mut_or_shared;
pub mod recording;
pub mod rendering;

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
