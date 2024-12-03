use crate::descriptor::mutable::MutDescExt;
use crate::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessFrame,
};
use crate::pipeline::compute_pipeline::BindlessComputePipeline;
use crate::platform::ash::ash_ext::DeviceExt;
use crate::platform::ash::{Ash, AshExecutingContext, AshPooledExecutionResource};
use crate::platform::{BindlessPipelinePlatform, RecordingCommandBuffer};
use ash::prelude::VkResult;
use ash::vk::{
	CommandBuffer, CommandBufferAllocateInfo, CommandBufferBeginInfo, CommandBufferLevel, CommandBufferUsageFlags,
	PipelineBindPoint, SubmitInfo,
};
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::{BindlessPushConstant, TransientAccess};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

pub struct AshRecordingContext<'a> {
	#[allow(dead_code)]
	frame: Arc<BindlessFrame<Ash>>,
	resource: AshPooledExecutionResource,
	_phantom: PhantomData<&'a ()>,
	// mut state
	cmd: CommandBuffer,
	compute_bind_descriptors: bool,
}

impl<'a> Deref for AshRecordingContext<'a> {
	type Target = AshPooledExecutionResource;

	fn deref(&self) -> &Self::Target {
		&self.resource
	}
}

impl<'a> AshRecordingContext<'a> {
	pub unsafe fn ash_record_and_execute<R>(
		bindless: &Arc<Bindless<Ash>>,
		f: impl FnOnce(&mut AshRecordingContext<'_>) -> VkResult<R>,
	) -> VkResult<AshExecutingContext<R>> {
		let mut recording = Self::new(bindless.frame(), bindless.execution_manager.pop())?;
		let r = f(&mut recording)?;
		Ok(AshExecutingContext::new(recording.ash_end_submit(), r))
	}

	pub fn new(frame: Arc<BindlessFrame<Ash>>, resource: AshPooledExecutionResource) -> VkResult<Self> {
		unsafe {
			let device = &resource.bindless.device;
			let cmd = device.allocate_command_buffer(
				&CommandBufferAllocateInfo::default()
					.command_pool(resource.command_pool)
					.level(CommandBufferLevel::PRIMARY)
					.command_buffer_count(1),
			)?;
			device.begin_command_buffer(
				cmd,
				&CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT),
			)?;
			Ok(Self {
				frame,
				resource,
				_phantom: PhantomData,
				cmd,
				compute_bind_descriptors: true,
			})
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
			let push_constant = BindlessPushConstant::new(desc.id(), 0);
			device.cmd_push_constants(
				self.cmd,
				self.bindless.global_descriptor_set().pipeline_layout,
				self.bindless.shader_stages,
				0,
				bytemuck::cast_slice(&[push_constant]),
			);
		}
	}

	pub unsafe fn ash_end_submit(self) -> AshPooledExecutionResource {
		unsafe {
			let device = &self.bindless.platform.device;
			device.end_command_buffer(self.cmd).unwrap();
			self.bindless.flush();
			device
				.queue_submit(
					self.bindless.queue,
					&[SubmitInfo::default()
						.command_buffers(&[self.cmd])
						.signal_semaphores(&[self.semaphore])],
					self.resource.fence,
				)
				.unwrap();
			self.resource
		}
	}
}

unsafe impl<'a> TransientAccess<'a> for AshRecordingContext<'a> {}

unsafe impl<'a> RecordingCommandBuffer<'a, Ash> for AshRecordingContext<'a> {
	fn dispatch<T: BufferStruct>(
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
}
