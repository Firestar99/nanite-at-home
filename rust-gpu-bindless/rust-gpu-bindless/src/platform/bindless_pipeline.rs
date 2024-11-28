use crate::descriptor::{Bindless, BindlessFrame};
use crate::pipeline::compute_pipeline::BindlessComputePipeline;
use crate::pipeline::shader::BindlessShader;
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader_type::ComputeShader;
use std::error::Error;
use std::ops::Deref;
use std::sync::Arc;

/// Internal interface for pipeline module related API calls, may change at any time!
pub unsafe trait BindlessPipelinePlatform: BindlessPlatform {
	type PipelineCreationError: 'static + Error;
	type ComputePipeline: 'static;
	type TraditionalGraphicsPipeline: 'static;
	type MeshGraphicsPipeline: 'static;
	type RecordingCommandBuffer<'a>: RecordingCommandBuffer<'a, Self>;
	type RecordingError: 'static + Error;
	type ExecutingCommandBuffer: ExecutingCommandBuffer<Self>;

	unsafe fn create_compute_pipeline<T: BufferStruct>(
		bindless: &Arc<Bindless<Self>>,
		compute_shader: &impl BindlessShader<ShaderType = ComputeShader, ParamConstant = T>,
	) -> Result<Self::ComputePipeline, Self::PipelineCreationError>;

	unsafe fn start_recording(
		bindless_frame: &Arc<BindlessFrame<Self>>,
	) -> Result<Self::RecordingCommandBuffer, Self::RecordingError>;
}

pub unsafe trait RecordingCommandBuffer<'a, P: BindlessPipelinePlatform>:
	Deref<Target = Arc<BindlessFrame<P>>>
{
	/// Dispatch a bindless compute shader
	unsafe fn dispatch<T: BufferStruct>(
		&mut self,
		pipeline: &Arc<BindlessComputePipeline<P, T>>,
		group_counts: [u32; 3],
		param: T,
	) -> Result<(), P::RecordingError>;

	fn submit(self) -> P::ExecutingCommandBuffer;
}

pub unsafe trait ExecutingCommandBuffer<P: BindlessPipelinePlatform> {}
