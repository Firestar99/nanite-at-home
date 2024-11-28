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
		bindless: &Arc<Bindless<P>>,
		compute_shader: &impl BindlessShader<ShaderType = ComputeShader, ParamConstant = T>,
	) -> Result<Arc<Self>, P::PipelineCreationError> {
		unsafe {
			Ok(Arc::new(Self {
				bindless: bindless.clone(),
				pipeline: P::create_compute_pipeline(&bindless, compute_shader)?,
				_phantom: PhantomData,
			}))
		}
	}
}
