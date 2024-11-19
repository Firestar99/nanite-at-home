use crate::descriptor::Bindless;
use crate::pipeline::execution_context::ExecutionContext;
use crate::pipeline::shader::BindlessShader;
use crate::platform::BindlessPipelinePlatform;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader_type::ComputeShader;
use std::marker::PhantomData;
use std::sync::Arc;

pub struct BindlessComputePipeline<P: BindlessPipelinePlatform, T: BufferStruct> {
	pub bindless: Arc<Bindless<P>>,
	pub pipeline: P::ComputePipeline,
	_phantom: PhantomData<T>,
}

impl<P: BindlessPipelinePlatform, T: BufferStruct> BindlessComputePipeline<P, T> {
	pub fn new(
		bindless: Arc<Bindless<P>>,
		stage: &impl BindlessShader<ShaderType = ComputeShader, ParamConstant = T>,
	) -> Result<Arc<Self>, P::PipelineCreationError> {
		// let layout = Self::verify_layout(&bindless, custom_layout).map_err(Validated::from)?;
		// let ci = ComputePipelineCreateInfo::stage_layout(specialize(&bindless, stage)?, layout);
		// unsafe {
		// 	Ok(Self::from(
		// 		VComputePipeline::new(bindless.device.clone(), cache, ci)?,
		// 		bindless,
		// 	))
		// }
	}
}

impl<'a, P: BindlessPipelinePlatform> ExecutionContext<'a, P> {
	/// Dispatch a bindless compute shader
	pub fn dispatch<T: BufferStruct>(
		mut self,
		pipeline: &Arc<BindlessComputePipeline<P, T>>,
		group_counts: [u32; 3],
		param: T,
	) -> Result<ExecutionContext<P>, P::RecordingError> {
		unsafe { P::cmd_dispatch(&mut self, pipeline, group_counts, param)? }
		Ok(self)
	}

	// /// Dispatch a bindless compute shader indirectly
	// pub fn dispatch_indirect<'a>(
	// 	&self,
	// 	cmd: &'a mut ExecutionContext,
	// 	indirect_buffer: Desc<MutOrRC, Buffer<DispatchIndirectCommand>>,
	// 	param: T,
	// ) -> Result<&'a mut ExecutionContext, Box<ValidationError>> {
	// 	unsafe { self.bind_modify(cmd, modify, param)?.dispatch_indirect(indirect_buffer) }
	// }
}
