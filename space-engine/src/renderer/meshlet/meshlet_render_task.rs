use crate::renderer::meshlet::instance_cull_compute::InstanceCullCompute;
use crate::renderer::meshlet::mesh_pipeline::MeshDrawPipeline;
use crate::renderer::meshlet::meshlet_allocation_buffer::MeshletAllocationBuffer;
use crate::renderer::render_graph::context::FrameContext;
use crate::renderer::Init;
use parking_lot::Mutex;
use space_asset_rt::meshlet::scene::MeshletSceneCpu;
use std::sync::Arc;
use vulkano::command_buffer::{
	CommandBufferBeginInfo, CommandBufferLevel, CommandBufferUsage, RecordingCommandBuffer, RenderingAttachmentInfo,
	RenderingInfo, SubpassContents,
};
use vulkano::format::{ClearValue, Format};
use vulkano::image::view::ImageView;
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::sync::GpuFuture;

pub struct MeshletRenderTask {
	init: Arc<Init>,
	alloc_buffer: MeshletAllocationBuffer,
	instance_cull_compute: InstanceCullCompute,
	mesh_pipeline: MeshDrawPipeline,
	pub scene: Mutex<Option<Arc<MeshletSceneCpu>>>,
}

// how many meshlet instances can be dynamically allocated, 1 << 17 = 131072
// about double what bistro needs if all meshlets rendered
const MESHLET_INSTANCE_CAPACITY: usize = 1 << 17;

impl MeshletRenderTask {
	pub fn new(
		init: &Arc<Init>,
		g_albedo_format_srgb: Format,
		g_normal_format: Format,
		g_roughness_metallic_format: Format,
		depth_format: Format,
	) -> Self {
		let alloc_buffer = MeshletAllocationBuffer::new(init, MESHLET_INSTANCE_CAPACITY);
		let instance_cull_compute = InstanceCullCompute::new(init, &alloc_buffer);
		let mesh_pipeline = MeshDrawPipeline::new(
			init,
			&alloc_buffer,
			g_albedo_format_srgb,
			g_normal_format,
			g_roughness_metallic_format,
			depth_format,
		);

		Self {
			init: init.clone(),
			alloc_buffer,
			mesh_pipeline,
			instance_cull_compute,
			scene: Mutex::new(None),
		}
	}

	#[profiling::function]
	pub fn record(
		&self,
		frame_context: &FrameContext,
		g_albedo: &Arc<ImageView>,
		g_normal: &Arc<ImageView>,
		g_roughness_metallic: &Arc<ImageView>,
		depth_image: &Arc<ImageView>,
		future: impl GpuFuture,
	) -> impl GpuFuture {
		let init = &self.init;
		let graphics = &init.queues.client.graphics_main;
		let scene = self.scene.lock().clone();
		self.alloc_buffer.reset();

		let mut cmd = RecordingCommandBuffer::new(
			init.cmd_buffer_allocator.clone(),
			graphics.queue_family_index(),
			CommandBufferLevel::Primary,
			CommandBufferBeginInfo {
				usage: CommandBufferUsage::OneTimeSubmit,
				..CommandBufferBeginInfo::default()
			},
		)
		.unwrap();

		if let Some(scene) = scene.as_ref() {
			self.instance_cull_compute
				.dispatch(frame_context, &mut cmd, &self.alloc_buffer, &scene);
		}

		cmd.begin_rendering(RenderingInfo {
			color_attachments: vec![
				Some(RenderingAttachmentInfo {
					load_op: AttachmentLoadOp::Clear,
					store_op: AttachmentStoreOp::Store,
					clear_value: Some(ClearValue::Float([0.0f32; 4])),
					..RenderingAttachmentInfo::image_view(g_albedo.clone())
				}),
				Some(RenderingAttachmentInfo {
					load_op: AttachmentLoadOp::Clear,
					store_op: AttachmentStoreOp::Store,
					clear_value: Some(ClearValue::Float([0.0f32; 4])),
					..RenderingAttachmentInfo::image_view(g_normal.clone())
				}),
				Some(RenderingAttachmentInfo {
					load_op: AttachmentLoadOp::Clear,
					store_op: AttachmentStoreOp::Store,
					clear_value: Some(ClearValue::Float([0.0f32; 4])),
					..RenderingAttachmentInfo::image_view(g_roughness_metallic.clone())
				}),
			],
			depth_attachment: Some(RenderingAttachmentInfo {
				load_op: AttachmentLoadOp::Clear,
				store_op: AttachmentStoreOp::Store,
				clear_value: Some(ClearValue::Depth(1.)),
				..RenderingAttachmentInfo::image_view(depth_image.clone())
			}),
			contents: SubpassContents::Inline,
			..RenderingInfo::default()
		})
		.unwrap();
		if let Some(scene) = scene.as_ref() {
			self.mesh_pipeline
				.draw(frame_context, &mut cmd, &self.alloc_buffer, &scene);
		}
		cmd.end_rendering().unwrap();

		let cmd = cmd.end().unwrap();
		future.then_execute(graphics.clone(), cmd).unwrap()
	}
}
