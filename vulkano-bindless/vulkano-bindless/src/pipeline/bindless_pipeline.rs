use crate::descriptor::Bindless;
use bytemuck::AnyBitPattern;
use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::command_buffer::RecordingCommandBuffer;
use vulkano::pipeline::{Pipeline, PipelineBindPoint, PipelineLayout};
use vulkano::{Validated, ValidationError, VulkanError};
use vulkano_bindless_shaders::desc_buffer::DescBuffer;
use vulkano_bindless_shaders::descriptor::metadata::PushConstant;

pub trait VulkanPipeline {
	type VulkanType: Pipeline;

	const BINDPOINT: PipelineBindPoint;

	fn bind_pipeline(
		cmd: &mut RecordingCommandBuffer,
		pipeline: Arc<Self::VulkanType>,
	) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>;
}

pub struct BindlessPipeline<Pipeline: VulkanPipeline, T: DescBuffer + AnyBitPattern> {
	pub bindless: Arc<Bindless>,
	pub(crate) pipeline: Arc<Pipeline::VulkanType>,
	_phantom: PhantomData<T>,
}

impl<Pipeline: VulkanPipeline, T: DescBuffer + AnyBitPattern> Clone for BindlessPipeline<Pipeline, T> {
	fn clone(&self) -> Self {
		Self {
			bindless: self.bindless.clone(),
			pipeline: self.pipeline.clone(),
			_phantom: PhantomData {},
		}
	}
}

impl<Pipeline: VulkanPipeline, T: DescBuffer + AnyBitPattern> Deref for BindlessPipeline<Pipeline, T> {
	type Target = Arc<Pipeline::VulkanType>;

	fn deref(&self) -> &Self::Target {
		&self.pipeline
	}
}

impl<Pipeline: VulkanPipeline, T: DescBuffer + AnyBitPattern> BindlessPipeline<Pipeline, T> {
	/// unsafely create a BindlessPipeline from a Pipeline
	///
	/// # Safety
	/// One must choose the correct T generic for this pipeline.
	pub unsafe fn from(pipeline: Arc<Pipeline::VulkanType>, bindless: Arc<Bindless>) -> Self {
		Self {
			bindless,
			pipeline,
			_phantom: PhantomData {},
		}
	}

	pub fn verify_layout(
		bindless: &Bindless,
		custom_layout: Option<Arc<PipelineLayout>>,
	) -> Result<Arc<PipelineLayout>, Validated<VulkanError>> {
		if let Some(layout) = custom_layout {
			match layout.set_layouts().get(0) {
				Some(set_0) if Arc::ptr_eq(set_0, bindless.descriptor_set.layout()) => {}
				_ => Err(Validated::from(Box::new(ValidationError {
					problem: "DescriptorSet 0 must be the bindless descriptor set".into(),
					..Default::default()
				})))?,
			}
			Ok(layout)
		} else {
			let push_constant_words = mem::size_of::<PushConstant<T>>().next_multiple_of(4) as u32;
			Ok(bindless
				.get_pipeline_layout(push_constant_words)
				.ok_or_else(|| {
					Validated::from(Box::new(ValidationError {
						problem: format!(
							"Bindless param T of word size {} is too large for minimum vulkan spec of 4",
							push_constant_words
						)
						.into(),
						..Default::default()
					}))
				})?
				.clone())
		}
	}

	/// Bind the pipeline, descriptor sets and push constants, making this pipeline ready to be drawn or dispatched. If
	/// additional descriptor sets are present, one may bind them after calling this function, before drawing.
	pub fn bind<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		param: impl DescBuffer<DescStatic = T>,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		Pipeline::bind_pipeline(cmd, self.pipeline.clone())?
			.bind_descriptor_sets(
				Pipeline::BINDPOINT,
				self.pipeline.layout().clone(),
				0,
				self.bindless.descriptor_set.clone(),
			)?
			.push_constants(self.pipeline.layout().clone(), 0, unsafe { param.to_static_desc() })
	}

	pub fn bind_modify<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: impl DescBuffer<DescStatic = T>,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		self.bind(cmd, param)?;
		modify(cmd)?;
		Ok(cmd)
	}
}

pub fn no_modify(
) -> impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>> {
	|cmd| Ok(cmd)
}
