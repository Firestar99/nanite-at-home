use crate::descriptor::MutDescExt;
use crate::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessFrame, BufferSlot,
	ImageSlot,
};
use crate::pipeline::access_buffer::MutBufferAccess;
use crate::pipeline::access_image::MutImageAccess;
use crate::pipeline::access_lock::AccessLockError;
use crate::pipeline::access_type::{
	BufferAccess, BufferAccessType, ImageAccess, ImageAccessType, TransferReadable, TransferWriteable,
};
use crate::pipeline::compute_pipeline::BindlessComputePipeline;
use crate::platform::ash::ash_ext::DeviceExt;
use crate::platform::ash::image_format::FormatExt;
use crate::platform::ash::{Ash, AshExecutingContext, AshPooledExecutionResource};
use crate::platform::{BindlessPipelinePlatform, RecordingContext, RecordingResourceContext};
use ash::prelude::VkResult;
use ash::vk::{
	BufferImageCopy2, BufferMemoryBarrier2, CommandBuffer, CommandBufferAllocateInfo, CommandBufferBeginInfo,
	CommandBufferLevel, CommandBufferUsageFlags, CopyBufferToImageInfo2, CopyImageToBufferInfo2, DependencyInfo,
	ImageMemoryBarrier2, ImageSubresourceLayers, ImageSubresourceRange, MemoryBarrier2, Offset3D, PipelineBindPoint,
	SubmitInfo, QUEUE_FAMILY_IGNORED, REMAINING_ARRAY_LAYERS, REMAINING_MIP_LEVELS, WHOLE_SIZE,
};
use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{BindlessPushConstant, ImageType, TransientAccess};
use smallvec::SmallVec;
use std::cell::RefCell;
use std::fmt::Debug;
use std::ops::Deref;
use std::sync::Arc;
use thiserror::Error;

pub struct AshRecordingContext<'a> {
	frame: Arc<BindlessFrame<Ash>>,
	resource: &'a AshRecordingResourceContext,
	exec_resource: AshPooledExecutionResource,
	// mut state
	cmd: CommandBuffer,
	compute_bind_descriptors: bool,
}

impl<'a> Deref for AshRecordingContext<'a> {
	type Target = AshPooledExecutionResource;

	fn deref(&self) -> &Self::Target {
		&self.exec_resource
	}
}

#[derive(Debug, Default)]
pub struct AshRecordingResourceContext {
	inner: RefCell<AshBarrierCollector>,
}

#[derive(Debug, Default)]
pub struct AshBarrierCollector {
	memory: SmallVec<[MemoryBarrier2<'static>; 0]>,
	buffers: SmallVec<[BufferMemoryBarrier2<'static>; 10]>,
	image: SmallVec<[ImageMemoryBarrier2<'static>; 10]>,
}

impl AshRecordingResourceContext {
	pub fn push_memory_barrier(&self, memory: MemoryBarrier2<'static>) {
		self.inner.borrow_mut().memory.push(memory);
	}

	pub fn push_buffer_barrier(&self, buffer: BufferMemoryBarrier2<'static>) {
		self.inner.borrow_mut().buffers.push(buffer);
	}

	pub fn push_image_barrier(&self, image: ImageMemoryBarrier2<'static>) {
		self.inner.borrow_mut().image.push(image);
	}
}

unsafe impl<'a> TransientAccess<'a> for &'a AshRecordingResourceContext {}

unsafe impl RecordingResourceContext<Ash> for AshRecordingResourceContext {
	unsafe fn to_transient_access(&self) -> impl TransientAccess<'_> {
		self
	}

	unsafe fn transition_buffer(&self, slot: &BufferSlot<Ash>, src: BufferAccess, dst: BufferAccess) {
		let src = src.to_ash_buffer_access();
		let dst = dst.to_ash_buffer_access();
		self.push_buffer_barrier(
			BufferMemoryBarrier2::default()
				.buffer(slot.buffer)
				.offset(0)
				.size(WHOLE_SIZE)
				.src_access_mask(src.access_mask)
				.src_stage_mask(src.stage_mask)
				.dst_access_mask(dst.access_mask)
				.dst_stage_mask(dst.stage_mask)
				.src_queue_family_index(QUEUE_FAMILY_IGNORED)
				.dst_queue_family_index(QUEUE_FAMILY_IGNORED),
		)
	}

	unsafe fn transition_image(&self, image: &ImageSlot<Ash>, src: ImageAccess, dst: ImageAccess) {
		let src = src.to_ash_image_access();
		let dst = dst.to_ash_image_access();
		self.push_image_barrier(
			ImageMemoryBarrier2::default()
				.image(image.image)
				.subresource_range(
					ImageSubresourceRange::default()
						// I'm unsure if it's valid to specify it like this or if the aspect has to match the format of
						// the image, I guess we'll find out later!
						.aspect_mask(image.format.aspect())
						.base_array_layer(0)
						.layer_count(REMAINING_ARRAY_LAYERS)
						.base_mip_level(0)
						.level_count(REMAINING_MIP_LEVELS),
				)
				.src_access_mask(src.access_mask)
				.src_stage_mask(src.stage_mask)
				.old_layout(src.image_layout)
				.dst_access_mask(dst.access_mask)
				.dst_stage_mask(dst.stage_mask)
				.new_layout(dst.image_layout)
				.src_queue_family_index(QUEUE_FAMILY_IGNORED)
				.dst_queue_family_index(QUEUE_FAMILY_IGNORED),
		)
	}
}

pub unsafe fn ash_record_and_execute<R>(
	bindless: &Arc<Bindless<Ash>>,
	f: impl FnOnce(&mut AshRecordingContext<'_>) -> Result<R, AshRecordingError>,
) -> Result<AshExecutingContext<R>, AshRecordingError> {
	let resource = AshRecordingResourceContext::default();
	let mut recording = AshRecordingContext::new(bindless.frame(), bindless.execution_manager.pop(), &resource)?;
	let r = f(&mut recording)?;
	Ok(AshExecutingContext::new(recording.ash_end_submit(), r))
}

impl<'a> AshRecordingContext<'a> {
	pub fn new(
		frame: Arc<BindlessFrame<Ash>>,
		exec_resource: AshPooledExecutionResource,
		resource: &'a AshRecordingResourceContext,
	) -> VkResult<Self> {
		unsafe {
			let device = &exec_resource.bindless.device;
			let cmd = device.allocate_command_buffer(
				&CommandBufferAllocateInfo::default()
					.command_pool(exec_resource.command_pool)
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
				exec_resource,
				cmd,
				compute_bind_descriptors: true,
			})
		}
	}

	/// Flushes the accumulated barriers as one [`Device::cmd_pipeline_barrier2`], must be called before any action
	/// command is recorded.
	pub fn ash_flush(&mut self) {
		unsafe {
			let device = &self.bindless.device;
			let mut collector = self.resource.inner.borrow_mut();
			device.cmd_pipeline_barrier2(
				self.cmd,
				&DependencyInfo::default()
					.memory_barriers(&*collector.memory)
					.buffer_memory_barriers(&*collector.buffers)
					.image_memory_barriers(&*collector.image),
			);
			collector.memory.clear();
			collector.buffers.clear();
			collector.image.clear();
		}
	}

	/// Invalidates internal state that keeps track of the command buffer's state. Currently, it forces the global
	/// descriptor set to be rebound again, in case anything overwrote it outside our control.
	pub fn ash_invalidate(&mut self) {
		self.compute_bind_descriptors = true;
	}

	/// Gets the command buffer to allow inserting custom ash commands directly.
	///
	/// # Safety
	/// Use [`Self::ash_flush`] and [`Self::ash_invalidate`] appropriately
	pub unsafe fn ash_get_command_buffer(&self) -> CommandBuffer {
		self.cmd
	}

	/// Gets the [`BindlessFrame`] which implements [`TransientAccess`] and this allows creating `TransientDesc`'s
	pub fn ash_get_bindless_frame(&self) -> &Arc<BindlessFrame<Ash>> {
		&self.frame
	}

	pub unsafe fn ash_bind_compute<T: BufferStruct>(
		&mut self,
		pipeline: &Arc<BindlessComputePipeline<Ash, T>>,
		param: T,
	) {
		unsafe {
			self.ash_flush();
			let device = &self.exec_resource.bindless.platform.device;
			device.cmd_bind_pipeline(self.cmd, PipelineBindPoint::COMPUTE, pipeline.pipeline.0.pipeline);
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
					self.exec_resource.fence,
				)
				.unwrap();
			self.exec_resource
		}
	}
}

unsafe impl<'a> TransientAccess<'a> for AshRecordingContext<'a> {}

unsafe impl<'a> RecordingContext<'a, Ash> for AshRecordingContext<'a> {
	fn resource_context(&self) -> &'a <Ash as BindlessPipelinePlatform>::RecordingResourceContext {
		self.resource
	}

	fn copy_buffer_to_image<
		BT: BufferContent + ?Sized,
		BA: BufferAccessType + TransferReadable,
		IT: ImageType,
		IA: ImageAccessType + TransferWriteable,
	>(
		&mut self,
		src_buffer: &mut MutBufferAccess<Ash, BT, BA>,
		dst_image: &mut MutImageAccess<Ash, IT, IA>,
	) {
		unsafe {
			self.ash_flush();
			let device = &self.bindless.platform.device;
			let buffer = src_buffer.inner_slot();
			let image = dst_image.inner_slot();
			device.cmd_copy_buffer_to_image2(
				self.cmd,
				&CopyBufferToImageInfo2::default()
					.src_buffer(buffer.buffer)
					.dst_image(image.image)
					.dst_image_layout(IA::IMAGE_ACCESS.to_ash_image_access().image_layout)
					.regions(&[BufferImageCopy2 {
						buffer_offset: 0,
						buffer_row_length: 0,
						buffer_image_height: 0,
						image_subresource: ImageSubresourceLayers {
							aspect_mask: image.format.aspect(),
							mip_level: 0,
							base_array_layer: 0,
							layer_count: image.array_layers,
						},
						image_offset: Offset3D::default(),
						image_extent: image.extent.into(),
						..Default::default()
					}]),
			)
		}
	}

	unsafe fn copy_image_to_buffer<
		IT: ImageType,
		IA: ImageAccessType + TransferReadable,
		BT: BufferContent + ?Sized,
		BA: BufferAccessType + TransferWriteable,
	>(
		&mut self,
		src_image: &mut MutImageAccess<Ash, IT, IA>,
		dst_buffer: &mut MutBufferAccess<Ash, BT, BA>,
	) {
		unsafe {
			self.ash_flush();
			let device = &self.bindless.platform.device;
			let buffer = dst_buffer.inner_slot();
			let image = src_image.inner_slot();
			device.cmd_copy_image_to_buffer2(
				self.cmd,
				&CopyImageToBufferInfo2::default()
					.src_image(image.image)
					.src_image_layout(IA::IMAGE_ACCESS.to_ash_image_access().image_layout)
					.dst_buffer(buffer.buffer)
					.regions(&[BufferImageCopy2 {
						buffer_offset: 0,
						buffer_row_length: 0,
						buffer_image_height: 0,
						image_subresource: ImageSubresourceLayers {
							aspect_mask: image.format.aspect(),
							mip_level: 0,
							base_array_layer: 0,
							layer_count: image.array_layers,
						},
						image_offset: Offset3D::default(),
						image_extent: image.extent.into(),
						..Default::default()
					}]),
			)
		}
	}

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

#[derive(Debug, Error)]
pub enum AshRecordingError {
	#[error("Vk Error: {0}")]
	Vk(ash::vk::Result),
	#[error("AccessLockError: {0}")]
	AccessLock(AccessLockError),
}

impl From<ash::vk::Result> for AshRecordingError {
	fn from(value: ash::vk::Result) -> Self {
		Self::Vk(value)
	}
}

impl From<AccessLockError> for AshRecordingError {
	fn from(value: AccessLockError) -> Self {
		Self::AccessLock(value)
	}
}
