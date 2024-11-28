use crate::descriptor::Bindless;
use crate::pipeline::compute_pipeline::BindlessComputePipeline;
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::buffer_content::{BufferStruct, Metadata};
use std::error::Error;
use std::sync::Arc;

/// Internal interface for pipeline module related API calls, may change at any time!
pub unsafe trait BindlessPipelinePlatform: BindlessPlatform {
	type PipelineCreationError: 'static + Error;
	type ComputePipeline: 'static;
	type TraditionalGraphicsPipeline: 'static;
	type MeshGraphicsPipeline: 'static;
	type RecordingCommandBuffer: RecordingCommandBuffer<Self>;
	type RecordingError: 'static + Error;
	type ExecutingCommandBuffer: ExecutingCommandBuffer<Self>;

	unsafe fn start_recording(
		bindless: &Arc<Bindless<Self>>,
		metadata: Metadata,
	) -> Result<Self::RecordingCommandBuffer, Self::RecordingError>;
}

pub unsafe trait RecordingCommandBuffer<P: BindlessPipelinePlatform>: Sized {
	/// Dispatch a bindless compute shader
	unsafe fn dispatch<T: BufferStruct>(
		self,
		pipeline: &Arc<BindlessComputePipeline<P, T>>,
		group_counts: [u32; 3],
		param: T,
	) -> Result<Self, P::RecordingError>;

	fn submit(self) -> P::ExecutingCommandBuffer;
}

pub unsafe trait ExecutingCommandBuffer<P: BindlessPipelinePlatform> {}
