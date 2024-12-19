use crate::descriptor::{Bindless, BufferSlot, ImageSlot};
use crate::pipeline::access_type::{BufferAccess, ImageAccess};
use crate::pipeline::compute_pipeline::BindlessComputePipeline;
use crate::pipeline::shader::BindlessShader;
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::TransientAccess;
use rust_gpu_bindless_shaders::shader_type::ComputeShader;
use std::error::Error;
use std::sync::Arc;

/// Internal interface for pipeline module related API calls, may change at any time!
pub unsafe trait BindlessPipelinePlatform: BindlessPlatform {
	type PipelineCreationError: 'static + Error + Send + Sync;
	type ComputePipeline: 'static + Send + Sync;
	type TraditionalGraphicsPipeline: 'static + Send + Sync;
	type MeshGraphicsPipeline: 'static + Send + Sync;
	type RecordingResourceContext: RecordingResourceContext<Self>;
	type RecordingContext<'a>: RecordingContext<'a, Self>;
	type RecordingError: 'static + Error + Send + Sync;
	type ExecutingContext<R: Send + Sync>: ExecutingContext<Self, R>;

	unsafe fn create_compute_pipeline<T: BufferStruct>(
		bindless: &Arc<Bindless<Self>>,
		compute_shader: &impl BindlessShader<ShaderType = ComputeShader, ParamConstant = T>,
	) -> Result<Self::ComputePipeline, Self::PipelineCreationError>;

	unsafe fn record_and_execute<R: Send + Sync>(
		bindless: &Arc<Bindless<Self>>,
		f: impl FnOnce(&mut Self::RecordingContext<'_>) -> Result<R, Self::RecordingError>,
	) -> Result<Self::ExecutingContext<R>, Self::RecordingError>;
}

pub unsafe trait RecordingContext<'a, P: BindlessPipelinePlatform>: TransientAccess<'a> {
	fn resource_context(&self) -> &'a P::RecordingResourceContext;

	/// Dispatch a bindless compute shader
	fn dispatch<T: BufferStruct>(
		&mut self,
		pipeline: &Arc<BindlessComputePipeline<P, T>>,
		group_counts: [u32; 3],
		param: T,
	) -> Result<(), P::RecordingError>;
}

pub unsafe trait RecordingResourceContext<P: BindlessPipelinePlatform>: 'static {
	unsafe fn to_transient_access(&self) -> impl TransientAccess<'_>;
	unsafe fn transition_buffer(&self, buffer: &BufferSlot<P>, src: BufferAccess, dst: BufferAccess);

	unsafe fn transition_image(&self, image: &ImageSlot<P>, src: ImageAccess, dst: ImageAccess);
}

pub unsafe trait ExecutingContext<P: BindlessPipelinePlatform, R: Send + Sync>: Send + Sync {
	/// Stopgap solution to wait for execution to finish
	fn block_on(self) -> R;
}
