use crate::descriptor::mutable::MutDescExt;
use crate::descriptor::{BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, RCDescExt};
use crate::pipeline::compute_pipeline::BindlessComputePipeline;
use crate::pipeline::execution_context::ExecutionContext;
use crate::platform::ash::ash_ext::DeviceExt;
use crate::platform::ash::{Ash, AshPooledExecutionResource};
use crate::platform::BindlessPipelinePlatform;
use ash::vk::{CommandBuffer, CommandBufferAllocateInfo, CommandBufferLevel, PipelineBindPoint};
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::BindlessPushConstant;
use std::sync::Arc;

unsafe impl BindlessPipelinePlatform for Ash {
	type PipelineCreationError = ash::vk::Result;
	type ComputePipeline = ash::vk::Pipeline;
	type TraditionalGraphicsPipeline = ash::vk::Pipeline;
	type MeshGraphicsPipeline = ash::vk::Pipeline;
	type RecordingCommandBuffer = RecordingCommandBuffer;
	type RecordingError = ash::vk::Result;

	unsafe fn cmd_start(
		exec: &mut ExecutionContext<Self>,
	) -> Result<Self::RecordingCommandBuffer, Self::RecordingError> {
		Ok(RecordingCommandBuffer::new(
			exec.bindless().platform.execution_resource_pool.pop(),
		))
	}

	unsafe fn cmd_submit(exec: &mut ExecutionContext<Self>) -> Result<(), Self::RecordingError> {
		exec.bindless().platform.device.end_command_buffer(exec.cmd.cmd)
	}

	unsafe fn cmd_dispatch<T: BufferStruct>(
		mut exec: &mut ExecutionContext<Self>,
		pipeline: &Arc<BindlessComputePipeline<Self, T>>,
		group_counts: [u32; 3],
		param: T,
	) -> Result<(), Self::RecordingError> {
		exec.setup_compute(pipeline, param);
		let device = &exec.bindless().platform.device;
		device.cmd_dispatch(exec.cmd.cmd, group_counts[0], group_counts[1], group_counts[2]);
		Ok(())
	}
}

pub struct RecordingCommandBuffer {
	resource: AshPooledExecutionResource,
	cmd: CommandBuffer,
	compute_bind_descriptors: bool,
}

impl RecordingCommandBuffer {
	pub fn new(resource: AshPooledExecutionResource) -> Self {
		unsafe {
			let cmd = resource
				.bindless
				.device
				.allocate_command_buffer(
					&CommandBufferAllocateInfo::default()
						.command_pool(resource.command_pool)
						.level(CommandBufferLevel::PRIMARY)
						.command_buffer_count(1),
				)
				.unwrap();
			Self {
				resource,
				cmd,
				compute_bind_descriptors: true,
			}
		}
	}
}

impl<'a> ExecutionContext<'a, Ash> {
	pub fn ash_flush(&mut self) {}

	pub unsafe fn ash_bind_compute<T: BufferStruct>(
		&mut self,
		pipeline: &Arc<BindlessComputePipeline<Ash, T>>,
		param: T,
	) {
		unsafe {
			self.ash_flush();
			let device = &self.bindless().platform.device;
			device.cmd_bind_pipeline(self.cmd.cmd, PipelineBindPoint::COMPUTE, pipeline.pipeline);
			if self.cmd.compute_bind_descriptors {
				self.cmd.compute_bind_descriptors = false;
				let desc = self.bindless().global_descriptor_set();
				unsafe {
					device.cmd_bind_descriptor_sets(
						self.cmd.cmd,
						PipelineBindPoint::COMPUTE,
						desc.pipeline_layout,
						0,
						&[desc.set],
						&[],
					);
				}
			}
			self.ash_push_param(param);
		}
	}

	/// A BumpAllocator would be nice to have, but this will do for now
	pub unsafe fn ash_push_param<T: BufferStruct>(&mut self, param: T) {
		unsafe {
			let device = &self.bindless().platform.device;
			let desc = self
				.bindless()
				.buffer()
				.alloc_from_data(
					&BindlessBufferCreateInfo {
						usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::STORAGE_BUFFER,
						name: "param",
						allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
					},
					param,
				)
				.unwrap();
			let push_constant = BindlessPushConstant::new(desc.id(), 0, self.metadata());
			device.cmd_push_constants(
				self.cmd.cmd,
				self.bindless().global_descriptor_set().pipeline_layout,
				self.bindless().shader_stages,
				0,
				bytemuck::cast_slice(&[push_constant]),
			);
		}
	}
}
