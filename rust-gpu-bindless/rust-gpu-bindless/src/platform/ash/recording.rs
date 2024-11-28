use crate::descriptor::mutable::MutDescExt;
use crate::descriptor::{BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessFrame};
use crate::pipeline::compute_pipeline::BindlessComputePipeline;
use crate::platform::ash::ash_ext::DeviceExt;
use crate::platform::ash::{Ash, AshExecutingCommandBuffer, AshPooledExecutionResource};
use crate::platform::{BindlessPipelinePlatform, RecordingCommandBuffer};
use ash::vk::{CommandBuffer, CommandBufferAllocateInfo, CommandBufferLevel, PipelineBindPoint, SubmitInfo};
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::BindlessPushConstant;
use std::ops::Deref;
use std::sync::Arc;

pub struct AshRecordingCommandBuffer {
	frame: Arc<BindlessFrame<Ash>>,
	resource: AshPooledExecutionResource,
	cmd: CommandBuffer,
	compute_bind_descriptors: bool,
}

impl Deref for AshRecordingCommandBuffer {
	type Target = AshPooledExecutionResource;

	fn deref(&self) -> &Self::Target {
		&self.resource
	}
}

impl AshRecordingCommandBuffer {
	pub fn new(frame: Arc<BindlessFrame<Ash>>, resource: AshPooledExecutionResource) -> Self {
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
				frame,
				resource,
				cmd,
				compute_bind_descriptors: true,
			}
		}
	}

	pub fn ash_flush(&mut self) {}

	pub unsafe fn ash_bind_compute<T: BufferStruct>(
		&mut self,
		pipeline: &Arc<BindlessComputePipeline<Ash, T>>,
		param: T,
	) {
		unsafe {
			self.ash_flush();
			let device = &self.resource.bindless.platform.device;
			device.cmd_bind_pipeline(self.cmd, PipelineBindPoint::COMPUTE, pipeline.pipeline);
			if self.compute_bind_descriptors {
				self.compute_bind_descriptors = false;
				let desc = self.bindless.global_descriptor_set();
				device.cmd_bind_descriptor_sets(
					self.cmd,
					PipelineBindPoint::COMPUTE,
					desc.pipeline_layout,
					0,
					&[desc.set],
					&[],
				);
			}
			self.ash_push_param(param);
		}
	}

	/// A BumpAllocator would be nice to have, but this will do for now
	pub unsafe fn ash_push_param<T: BufferStruct>(&mut self, param: T) {
		unsafe {
			let device = &self.bindless.platform.device;
			let desc = self
				.bindless
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
			let push_constant = BindlessPushConstant::new(desc.id(), 0, self.frame.metadata);
			device.cmd_push_constants(
				self.cmd,
				self.bindless.global_descriptor_set().pipeline_layout,
				self.bindless.shader_stages,
				0,
				bytemuck::cast_slice(&[push_constant]),
			);
		}
	}
}

unsafe impl RecordingCommandBuffer<Ash> for AshRecordingCommandBuffer {
	unsafe fn dispatch<T: BufferStruct>(
		&mut self,
		pipeline: &Arc<BindlessComputePipeline<Ash, T>>,
		group_counts: [u32; 3],
		param: T,
	) -> Result<(), <Ash as BindlessPipelinePlatform>::RecordingError> {
		unsafe {
			self.ash_bind_compute(pipeline, param);
			let device = &self.bindless.platform.device;
			device.cmd_dispatch(self.cmd, group_counts[0], group_counts[1], group_counts[2]);
			Ok(())
		}
	}

	// /// Dispatch a bindless compute shader indirectly
	// fn dispatch_indirect<T: BufferStruct>(
	// 	mut self,
	// 	pipeline: &Arc<BindlessComputePipeline<Self, T>>,
	// 	indirect_buffer: Desc<MutOrRC, Buffer<DispatchIndirectCommand>>,
	// 	param: T,
	// ) -> Result<Self, Ash::RecordingError> {
	// 	unsafe {
	// 		self.ash_bind_compute(pipeline, param);
	// 		let device = &self.bindless.platform.device;
	// 		device.cmd_dispatch_indirect(self.cmd, indirect_buffer, 0);
	// 		Ok(self)
	// 	}
	// }

	fn submit(self) -> AshExecutingCommandBuffer {
		unsafe {
			let device = &self.bindless.platform.device;
			device.end_command_buffer(self.cmd).unwrap();
			device
				.queue_submit(
					self.bindless.queue,
					&[SubmitInfo::default()
						.command_buffers(&[self.cmd])
						.signal_semaphores(&[self.semaphore])],
					self.resource.fence,
				)
				.unwrap();
			AshExecutingCommandBuffer::new(self.resource)
		}
	}
}
