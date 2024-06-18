use crate::space::renderer::meshlet::indices::triangle_indices_write_vec;
use crate::space::renderer::meshlet::mesh_pipeline::MeshDrawPipeline;
use crate::space::renderer::meshlet::offset::MeshletOffset;
use crate::space::renderer::meshlet::scene::MeshletInstance;
use crate::space::renderer::render_graph::context::FrameContext;
use crate::space::Init;
use glam::{vec3, Affine3A};
use space_asset::meshlet::mesh::{MeshletCpuMesh, MeshletData, MeshletMesh, MeshletVertex};
use std::iter::repeat;
use std::sync::Arc;
use vulkano::buffer::{BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{
	CommandBufferBeginInfo, CommandBufferLevel, CommandBufferUsage, RecordingCommandBuffer, RenderingAttachmentInfo,
	RenderingInfo, SubpassContents,
};
use vulkano::format::{ClearValue, Format};
use vulkano::image::view::ImageView;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::sync::GpuFuture;
use vulkano_bindless::descriptor::reference::Strong;
use vulkano_bindless::descriptor::{Buffer, RCDesc, RCDescExt};

pub struct MeshletRenderTask {
	init: Arc<Init>,
	pipeline_mesh: MeshDrawPipeline,
	mesh: MeshletCpuMesh<Strong>,
	instances: RCDesc<Buffer<[MeshletInstance<Strong>]>>,
}

impl MeshletRenderTask {
	pub fn new(init: &Arc<Init>, format_color: Format, format_depth: Format) -> Self {
		let pipeline_mesh = MeshDrawPipeline::new(init, format_color, format_depth);

		let mesh_cpu;
		let instances;
		{
			let alloc_info = AllocationCreateInfo {
				memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
				..AllocationCreateInfo::default()
			};
			let buffer_info = BufferCreateInfo {
				usage: BufferUsage::STORAGE_BUFFER,
				..BufferCreateInfo::default()
			};

			let quads = [
				vec3(0., 0., 0.),
				vec3(0., 0., -1.),
				vec3(0., 3., 0.),
				vec3(3., 0., 0.),
				vec3(2., 2., 0.),
			];

			let vertices = init
				.bindless
				.buffer()
				.alloc_from_iter(
					init.memory_allocator.clone(),
					buffer_info.clone(),
					alloc_info.clone(),
					quads
						.iter()
						.copied()
						.flat_map(|quad| {
							[
								MeshletVertex::new(quad),
								MeshletVertex::new(quad + vec3(1., 0., 0.)),
								MeshletVertex::new(quad + vec3(0., 1., 0.)),
								MeshletVertex::new(quad + vec3(1., 1., 0.)),
							]
						})
						.collect::<Vec<_>>(),
				)
				.unwrap();

			let indices = init
				.bindless
				.buffer()
				.alloc_from_iter(
					init.memory_allocator.clone(),
					buffer_info.clone(),
					alloc_info.clone(),
					triangle_indices_write_vec(
						repeat([0, 1, 2, 1, 2, 3])
							.take(quads.len())
							.flatten()
							.collect::<Vec<_>>()
							.into_iter(),
					),
				)
				.unwrap();

			let meshlets = init
				.bindless
				.buffer()
				.alloc_from_iter(
					init.memory_allocator.clone(),
					buffer_info.clone(),
					alloc_info.clone(),
					quads.iter().enumerate().map(|(i, _)| MeshletData {
						vertex_offset: MeshletOffset::new(i * 4, 4),
						triangle_indices_offset: MeshletOffset::new(i * 2, 2),
					}),
				)
				.unwrap();

			let mesh = init
				.bindless
				.buffer()
				.alloc_from_data(
					init.memory_allocator.clone(),
					buffer_info.clone(),
					alloc_info.clone(),
					MeshletMesh {
						vertices: vertices.to_strong(),
						triangle_indices: indices.to_strong(),
						meshlets: meshlets.to_strong(),
						num_meshlets: meshlets.len() as u32,
					},
				)
				.unwrap();
			instances = init
				.bindless
				.buffer()
				.alloc_from_iter(
					init.memory_allocator.clone(),
					buffer_info.clone(),
					alloc_info.clone(),
					[MeshletInstance::new(mesh.to_strong(), Affine3A::default())],
				)
				.unwrap();
			mesh_cpu = MeshletCpuMesh {
				mesh,
				num_meshlets: meshlets.len() as u32,
			};
		}

		Self {
			init: init.clone(),
			pipeline_mesh,
			mesh: mesh_cpu,
			instances,
		}
	}

	pub fn record(
		&self,
		frame_context: &FrameContext,
		output_image: &Arc<ImageView>,
		depth_image: &Arc<ImageView>,
		future: impl GpuFuture,
	) -> impl GpuFuture {
		let init = &self.init;
		let graphics = &init.queues.client.graphics_main;

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
		cmd.begin_rendering(RenderingInfo {
			color_attachments: vec![Some(RenderingAttachmentInfo {
				load_op: AttachmentLoadOp::Clear,
				store_op: AttachmentStoreOp::Store,
				clear_value: Some(ClearValue::Float([0.0f32; 4])),
				..RenderingAttachmentInfo::image_view(output_image.clone())
			})],
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
		self.pipeline_mesh
			.draw(frame_context, &mut cmd, &self.mesh, &self.instances);
		cmd.end_rendering().unwrap();
		let cmd = cmd.end().unwrap();

		future.then_execute(graphics.clone(), cmd).unwrap()
	}
}
