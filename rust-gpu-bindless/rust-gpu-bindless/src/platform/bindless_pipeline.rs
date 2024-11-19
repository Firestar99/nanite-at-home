use crate::pipeline::compute_pipeline::BindlessComputePipeline;
use crate::pipeline::execution_context::ExecutionContext;
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use std::error::Error;
use std::sync::Arc;

/// Internal interface for pipeline module related API calls, may change at any time!
pub unsafe trait BindlessPipelinePlatform: BindlessPlatform {
	type PipelineCreationError: 'static + Error;
	type ComputePipeline: 'static;
	type TraditionalGraphicsPipeline: 'static;
	type MeshGraphicsPipeline: 'static;
	type RecordingCommandBuffer: 'static;
	type RecordingError: 'static + Error;

	unsafe fn cmd_start(
		exec: &mut ExecutionContext<Self>,
	) -> Result<Self::RecordingCommandBuffer, Self::RecordingError>;

	unsafe fn cmd_submit(exec: &mut ExecutionContext<Self>) -> Result<(), Self::RecordingError>;

	unsafe fn cmd_dispatch<T: BufferStruct>(
		exec: &mut ExecutionContext<Self>,
		pipeline: &Arc<BindlessComputePipeline<Self, T>>,
		group_counts: [u32; 3],
		param: T,
	) -> Result<(), Self::RecordingError>;
}
