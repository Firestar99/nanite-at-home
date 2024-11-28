use crate::descriptor::Bindless;
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
