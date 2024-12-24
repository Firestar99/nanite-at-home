use crate::descriptor::Bindless;
use crate::pipeline::shader::BindlessShader;
use crate::platform::BindlessPipelinePlatform;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader_type::ComputeShader;
use std::marker::PhantomData;
use std::sync::Arc;

impl<P: BindlessPipelinePlatform> Bindless<P> {
	pub fn create_compute_pipeline<T: BufferStruct>(
		self: &Arc<Self>,
		compute_shader: &impl BindlessShader<ShaderType = ComputeShader, ParamConstant = T>,
	) -> Result<BindlessComputePipeline<P, T>, P::PipelineCreationError> {
		BindlessComputePipeline::new(self, compute_shader)
	}
}

#[derive(Debug, Clone)]
pub struct BindlessComputePipeline<P: BindlessPipelinePlatform, T: BufferStruct> {
	pipeline: Arc<P::ComputePipeline>,
	_phantom: PhantomData<T>,
}

impl<P: BindlessPipelinePlatform, T: BufferStruct> BindlessComputePipeline<P, T> {
	pub fn new(
		bindless: &Arc<Bindless<P>>,
		compute_shader: &impl BindlessShader<ShaderType = ComputeShader, ParamConstant = T>,
	) -> Result<Self, P::PipelineCreationError> {
		unsafe {
			Ok(Self {
				pipeline: Arc::new(P::create_compute_pipeline(&bindless, compute_shader)?),
				_phantom: PhantomData,
			})
		}
	}

	pub fn inner(&self) -> &Arc<P::ComputePipeline> {
		&self.pipeline
	}
}
