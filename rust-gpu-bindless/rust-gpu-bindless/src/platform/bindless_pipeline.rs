use crate::descriptor::Bindless;
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
	type PipelineCreationError: 'static + Error;
	type ComputePipeline: 'static;
	type TraditionalGraphicsPipeline: 'static;
	type MeshGraphicsPipeline: 'static;
	type RecordingContext<'a>: RecordingCommandBuffer<'a, Self>;
	type RecordingError: 'static + Error;
	type ExecutingContext<R>: ExecutingCommandBuffer<Self, R>;

	unsafe fn create_compute_pipeline<T: BufferStruct>(
		bindless: &Arc<Bindless<Self>>,
		compute_shader: &impl BindlessShader<ShaderType = ComputeShader, ParamConstant = T>,
	) -> Result<Self::ComputePipeline, Self::PipelineCreationError>;

	unsafe fn record_and_execute<R>(
		bindless: &Arc<Bindless<Self>>,
		f: impl FnOnce(&mut Self::RecordingContext<'_>) -> Result<R, Self::RecordingError>,
	) -> Result<Self::ExecutingContext<R>, Self::RecordingError>;
}

pub unsafe trait RecordingCommandBuffer<'a, P: BindlessPipelinePlatform>: TransientAccess<'a> {
	/// Dispatch a bindless compute shader
	fn dispatch<T: BufferStruct>(
		&mut self,
		pipeline: &Arc<BindlessComputePipeline<P, T>>,
		group_counts: [u32; 3],
		param: T,
	) -> Result<(), P::RecordingError>;
}

pub unsafe trait ExecutingCommandBuffer<P: BindlessPipelinePlatform, R> {
	/// Stopgap solution to wait for execution to finish
	fn block_on(self) -> R;
}
