use crate::descriptor::MutDescExt;
use crate::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BufferSlot, ImageSlot,
};
use crate::pipeline::{
	BindlessComputePipeline, BufferAccess, BufferAccessType, HasResourceContext, ImageAccess, ImageAccessType,
	IndirectCommandReadable, MutBufferAccess, MutImageAccess, MutOrSharedBuffer, Recording, RecordingError,
	TransferReadable, TransferWriteable,
};
use crate::platform::ash::image_format::FormatExt;
use crate::platform::ash::{Ash, AshExecution, AshPendingExecution};
use crate::platform::{BindlessPipelinePlatform, RecordingContext, RecordingResourceContext};
use ash::vk::{
	BufferCopy, BufferImageCopy2, BufferMemoryBarrier2, CommandBuffer, CommandBufferBeginInfo, CommandBufferUsageFlags,
	CopyBufferToImageInfo2, CopyImageToBufferInfo2, DependencyInfo, Fence, ImageMemoryBarrier2, ImageSubresourceLayers,
	ImageSubresourceRange, MemoryBarrier2, Offset3D, PipelineBindPoint, PipelineStageFlags, QUEUE_FAMILY_IGNORED,
	REMAINING_ARRAY_LAYERS, REMAINING_MIP_LEVELS, SubmitInfo, TimelineSemaphoreSubmitInfo, WHOLE_SIZE,
};
use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{BindlessPushConstant, ImageType, TransientAccess};
use smallvec::SmallVec;
use std::cell::RefCell;
use std::fmt::Debug;
use std::sync::Arc;
use thiserror::Error;

pub struct AshRecordingResourceContext {
	inner: RefCell<AshBarrierCollector>,
	pub(super) execution: Arc<AshExecution>,
	dependencies: RefCell<SmallVec<[AshPendingExecution; 4]>>,
}

#[derive(Debug, Clone, Default)]
pub struct AshBarrierCollector {
	memory: SmallVec<[MemoryBarrier2<'static>; 0]>,
	buffers: SmallVec<[BufferMemoryBarrier2<'static>; 10]>,
	image: SmallVec<[ImageMemoryBarrier2<'static>; 10]>,
}

impl AshBarrierCollector {
	pub fn is_empty(&self) -> bool {
		self.memory.is_empty() && self.buffers.is_empty() && self.image.is_empty()
	}
}

impl AshRecordingResourceContext {
	pub fn new(execution: Arc<AshExecution>) -> Self {
		Self {
			inner: RefCell::new(AshBarrierCollector::default()),
			execution,
			dependencies: RefCell::new(SmallVec::new()),
		}
	}

	/// Gets the [`AshExecution`] of this execution
	pub fn ash_execution(&self) -> &Arc<AshExecution> {
		&self.execution
	}

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
	fn to_transient_access(&self) -> impl TransientAccess<'_> {
		self
	}

	fn add_dependency(&self, pending: AshPendingExecution) {
		// TODO is it efficient to check here if dependencies completed?
		self.dependencies.borrow_mut().push(pending);
	}

	fn to_pending_execution(&self) -> AshPendingExecution {
		AshPendingExecution::new(&self.execution)
	}

	unsafe fn transition_buffer(&self, buffer: &BufferSlot<Ash>, src: BufferAccess, dst: BufferAccess) {
		let src = src.to_ash_buffer_access();
		let dst = dst.to_ash_buffer_access();
		self.push_buffer_barrier(
			BufferMemoryBarrier2::default()
				.buffer(buffer.buffer)
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
	bindless: &Bindless<Ash>,
	f: impl FnOnce(&mut Recording<'_, Ash>) -> Result<R, RecordingError<Ash>>,
) -> Result<R, RecordingError<Ash>> {
	unsafe {
		let resource = AshRecordingResourceContext::new(
			bindless
				.execution_manager
				.new_execution()
				.map_err(AshRecordingError::from)?,
		);
		let mut recording = Recording::new(AshRecordingContext::new(&resource)?);
		let r = f(&mut recording)?;
		let cmd = recording.into_inner().ash_end()?;
		ash_submit(bindless, resource, cmd)?;
		Ok(r)
	}
}

pub unsafe fn ash_submit(
	bindless: &Bindless<Ash>,
	resource_context: AshRecordingResourceContext,
	cmd: CommandBuffer,
) -> Result<(), AshRecordingError> {
	unsafe {
		let device = &bindless.platform.device;
		// Safety: dependencies keeps the semaphores alive
		let dependencies = resource_context
			.dependencies
			.into_inner()
			.iter()
			.filter_map(|a| a.upgrade_ash_resource())
			.filter(|a| !a.completed())
			.collect::<SmallVec<[_; 4]>>();
		let wait_semaphores = dependencies
			.iter()
			.map(|d| d.resource().semaphore)
			.collect::<SmallVec<[_; 4]>>();
		let wait_dst_stage_mask = dependencies
			.iter()
			// TODO ALL_COMMANDS should not be necessary here if we bind AccessFlags to PendingExecution
			.map(|_| PipelineStageFlags::ALL_COMMANDS)
			.collect::<SmallVec<[_; 4]>>();
		let wait_values = dependencies
			.iter()
			.map(|d| d.resource().timeline_value)
			.collect::<SmallVec<[_; 4]>>();

		bindless.flush();

		{
			let execution_resource = resource_context.execution.resource();
			let queue = bindless.queue.lock();
			device.queue_submit(
				*queue,
				&[SubmitInfo::default()
					.command_buffers(&[cmd])
					.wait_semaphores(&wait_semaphores)
					.wait_dst_stage_mask(&wait_dst_stage_mask)
					.signal_semaphores(&[execution_resource.semaphore])
					.push_next(
						&mut TimelineSemaphoreSubmitInfo::default()
							.wait_semaphore_values(&wait_values)
							.signal_semaphore_values(&[execution_resource.timeline_value]),
					)],
				Fence::null(),
			)?;
		}

		bindless
			.execution_manager
			.submit_for_waiting(resource_context.execution)?;
		Ok(())
	}
}

pub struct AshRecordingContext<'a> {
	/// The same bindless as `self.execution.frame.bindless` but with only 1 instead of 3 indirections.
	/// Also less typing.
	pub(super) bindless: Bindless<Ash>,
	pub(super) resource_context: &'a AshRecordingResourceContext,
	// mut state
	pub(super) cmd: CommandBuffer,
	compute_bind_descriptors: bool,
}

impl<'a> AshRecordingContext<'a> {
	pub fn new(resource_context: &'a AshRecordingResourceContext) -> Result<Self, AshRecordingError> {
		unsafe {
			let bindless = resource_context.execution.bindless().clone();
			let device = &bindless.device;
			let cmd = resource_context.execution.resource().command_buffer;
			device.begin_command_buffer(
				cmd,
				&CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT),
			)?;
			Ok(Self {
				bindless,
				resource_context,
				cmd,
				compute_bind_descriptors: true,
			})
		}
	}

	/// Gets the command buffer to allow inserting custom ash commands directly.
	///
	/// # Safety
	/// Use [`Self::ash_flush`] and [`Self::ash_invalidate`] appropriately
	pub unsafe fn ash_command_buffer(&self) -> CommandBuffer {
		self.cmd
	}

	/// Gets the [`AshExecution`] of this execution
	pub fn ash_execution(&self) -> &Arc<AshExecution> {
		&self.resource_context.execution
	}

	/// Flushes the following state changes to the command buffer:
	/// * accumulated barriers
	pub fn ash_flush(&mut self) {
		self.ash_flush_barriers();
	}

	/// Flushes the accumulated barriers as one [`Device::cmd_pipeline_barrier2`], must be called before any action
	/// command is recorded.
	pub fn ash_flush_barriers(&mut self) {
		unsafe {
			let mut collector = self.resource_context.inner.borrow_mut();
			if collector.is_empty() {
				return;
			}

			let device = &self.bindless.device;
			device.cmd_pipeline_barrier2(
				self.cmd,
				&DependencyInfo::default()
					.memory_barriers(&collector.memory)
					.buffer_memory_barriers(&collector.buffers)
					.image_memory_barriers(&collector.image),
			);
			collector.memory.clear();
			collector.buffers.clear();
			collector.image.clear();
		}
	}

	/// Return an Error if any barrier flushes are queued. Useful for verifying no flushes happen within a render pass.
	pub fn ash_must_not_flush_barriers(&self) -> Result<(), AshRecordingError> {
		let collector = self.resource_context.inner.borrow();
		if collector.is_empty() {
			Ok(())
		} else {
			Err(AshRecordingError::BarrierWhileRendering {
				collector: Box::new(collector.clone()),
			})
		}
	}

	/// Invalidates internal state that keeps track of the command buffer's state. Currently, it forces the global
	/// descriptor set to be rebound again, in case anything overwrote it outside our control.
	pub fn ash_invalidate(&mut self) {
		self.compute_bind_descriptors = true;
	}

	pub unsafe fn ash_bind_compute<T: BufferStruct>(&mut self, pipeline: &BindlessComputePipeline<Ash, T>, param: T) {
		unsafe {
			self.ash_flush();
			let device = &self.bindless.platform.device;
			device.cmd_bind_pipeline(self.cmd, PipelineBindPoint::COMPUTE, pipeline.inner().0.pipeline);
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

	pub unsafe fn ash_end(mut self) -> Result<CommandBuffer, AshRecordingError> {
		unsafe {
			self.ash_flush();
			let device = &self.bindless.platform.device;
			device.end_command_buffer(self.cmd)?;
			Ok(self.cmd)
		}
	}
}

unsafe impl<'a> TransientAccess<'a> for AshRecordingContext<'a> {}

unsafe impl<'a> HasResourceContext<'a, Ash> for AshRecordingContext<'a> {
	#[inline]
	fn bindless(&self) -> &Bindless<Ash> {
		&self.bindless
	}

	#[inline]
	fn resource_context(&self) -> &'a <Ash as BindlessPipelinePlatform>::RecordingResourceContext {
		self.resource_context
	}
}

unsafe impl<'a> RecordingContext<'a, Ash> for AshRecordingContext<'a> {
	unsafe fn copy_buffer_to_buffer<
		T: BufferStruct,
		SA: BufferAccessType + TransferReadable,
		DA: BufferAccessType + TransferWriteable,
	>(
		&mut self,
		src: impl MutOrSharedBuffer<Ash, T, SA>,
		dst: &MutBufferAccess<Ash, T, DA>,
	) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_flush();
			let device = &self.bindless.platform.device;
			let src = src.inner_slot();
			let dst = dst.inner_slot();
			device.cmd_copy_buffer(
				self.cmd,
				src.buffer,
				dst.buffer,
				&[BufferCopy {
					src_offset: 0,
					dst_offset: 0,
					size: src.size,
				}],
			);
			Ok(())
		}
	}

	unsafe fn copy_buffer_to_buffer_slice<
		T: BufferStruct,
		SA: BufferAccessType + TransferReadable,
		DA: BufferAccessType + TransferWriteable,
	>(
		&mut self,
		src: impl MutOrSharedBuffer<Ash, [T], SA>,
		dst: &MutBufferAccess<Ash, [T], DA>,
	) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_flush();
			let device = &self.bindless.platform.device;
			let src = src.inner_slot();
			let dst = dst.inner_slot();
			device.cmd_copy_buffer(
				self.cmd,
				src.buffer,
				dst.buffer,
				&[BufferCopy {
					src_offset: 0,
					dst_offset: 0,
					size: src.size,
				}],
			);
			Ok(())
		}
	}

	unsafe fn copy_buffer_to_image<
		BT: BufferContent + ?Sized,
		BA: BufferAccessType + TransferReadable,
		IT: ImageType,
		IA: ImageAccessType + TransferWriteable,
	>(
		&mut self,
		src_buffer: &MutBufferAccess<Ash, BT, BA>,
		dst_image: &MutImageAccess<Ash, IT, IA>,
	) -> Result<(), AshRecordingError> {
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
			);
			Ok(())
		}
	}

	unsafe fn copy_image_to_buffer<
		IT: ImageType,
		IA: ImageAccessType + TransferReadable,
		BT: BufferContent + ?Sized,
		BA: BufferAccessType + TransferWriteable,
	>(
		&mut self,
		src_image: &MutImageAccess<Ash, IT, IA>,
		dst_buffer: &MutBufferAccess<Ash, BT, BA>,
	) -> Result<(), AshRecordingError> {
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
			);
			Ok(())
		}
	}

	unsafe fn dispatch<T: BufferStruct>(
		&mut self,
		pipeline: &BindlessComputePipeline<Ash, T>,
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

	unsafe fn dispatch_indirect<T: BufferStruct, A: BufferAccessType + IndirectCommandReadable>(
		&mut self,
		pipeline: &BindlessComputePipeline<Ash, T>,
		indirect: impl MutOrSharedBuffer<Ash, [u32; 3], A>,
		param: T,
	) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_bind_compute(pipeline, param);
			let device = &self.bindless.platform.device;
			device.cmd_dispatch_indirect(self.cmd, indirect.inner_slot().buffer, 0);
			Ok(())
		}
	}
}

#[derive(Error)]
pub enum AshRecordingError {
	#[error("Vk Error: {0}")]
	Vk(#[from] ash::vk::Result),
	#[error("No barriers must be inserted while rendering: {collector:?}")]
	BarrierWhileRendering { collector: Box<AshBarrierCollector> },
}

impl Debug for AshRecordingError {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		core::fmt::Display::fmt(self, f)
	}
}

impl From<AshRecordingError> for RecordingError<Ash> {
	fn from(value: AshRecordingError) -> Self {
		RecordingError::Platform(value)
	}
}
