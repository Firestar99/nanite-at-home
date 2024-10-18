use crate::descriptor::Bindless;
use crate::pipeline::bindless_pipeline::{BindlessPipeline, VulkanPipeline};
use crate::pipeline::shader::BindlessShader;
use crate::pipeline::specialize::specialize;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader_type::ComputeShader;
use std::sync::Arc;
use vulkano::buffer::Subbuffer;
use vulkano::command_buffer::{DispatchIndirectCommand, RecordingCommandBuffer};
use vulkano::pipeline::cache::PipelineCache;
use vulkano::pipeline::compute::ComputePipelineCreateInfo;
use vulkano::pipeline::ComputePipeline as VComputePipeline;
use vulkano::pipeline::{PipelineBindPoint, PipelineLayout};
use vulkano::{Validated, ValidationError, VulkanError};

pub type BindlessComputePipeline<T> = BindlessPipeline<ComputePipelineType, T>;

pub struct ComputePipelineType;

impl VulkanPipeline for ComputePipelineType {
	type VulkanType = VComputePipeline;
	const BINDPOINT: PipelineBindPoint = PipelineBindPoint::Compute;

	fn bind_pipeline(
		cmd: &mut RecordingCommandBuffer,
		pipeline: Arc<Self::VulkanType>,
	) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>> {
		cmd.bind_pipeline_compute(pipeline)
	}
}

impl<T: BufferStruct> BindlessComputePipeline<T> {
	pub fn new(
		bindless: Arc<Bindless>,
		stage: &impl BindlessShader<ShaderType = ComputeShader, ParamConstant = T>,
		cache: Option<Arc<PipelineCache>>,
		custom_layout: Option<Arc<PipelineLayout>>,
	) -> Result<Self, Validated<VulkanError>> {
		let layout = Self::verify_layout(&bindless, custom_layout).map_err(Validated::from)?;
		let ci = ComputePipelineCreateInfo::stage_layout(specialize(&bindless, stage)?, layout);
		unsafe {
			Ok(Self::from(
				VComputePipeline::new(bindless.device.clone(), cache, ci)?,
				bindless,
			))
		}
	}

	/// Dispatch a bindless compute shader
	///
	/// # Safety
	///
	/// - The general [shader safety requirements](vulkano::shader#safety) apply.
	pub unsafe fn dispatch<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		group_counts: [u32; 3],
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: T,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		unsafe { self.bind_modify(cmd, modify, param)?.dispatch(group_counts) }
	}

	/// Dispatch a bindless compute shader indirectly
	///
	/// # Safety
	///
	/// - The general [shader safety requirements](crate::shader#safety) apply.
	/// - The [safety requirements for `DispatchIndirectCommand`](DispatchIndirectCommand#safety)
	///   apply.
	pub unsafe fn dispatch_indirect<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		indirect_buffer: Subbuffer<[DispatchIndirectCommand]>,
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: T,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		unsafe { self.bind_modify(cmd, modify, param)?.dispatch_indirect(indirect_buffer) }
	}
}
