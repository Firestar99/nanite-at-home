use crate::space::renderer::meshlet::indices::triangle_indices_write_vec;
use crate::space::renderer::meshlet::mesh_pipeline::MeshDrawPipeline;
use crate::space::renderer::meshlet::offset::MeshletOffset;
use crate::space::renderer::render_graph::context::FrameContext;
use crate::space::Init;
use glam::{vec3, Affine3A};
use space_asset::meshlet::instance::MeshletInstance;
use space_asset::meshlet::mesh::{MeshletData, MeshletMesh};
use space_asset::meshlet::mesh2instance::{MeshletMesh2Instance, MeshletMesh2InstanceCpu};
use space_asset::meshlet::vertex::MeshletDrawVertex;
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
use vulkano_bindless::descriptor::RCDescExt;

pub struct MeshletRenderTask {
	init: Arc<Init>,
	pipeline_mesh: MeshDrawPipeline,
	mesh2instances: Vec<MeshletMesh2InstanceCpu>,
}

fn upload_test_mesh(init: &Arc<Init>) -> MeshletMesh2InstanceCpu {
	let alloc_info = AllocationCreateInfo {
		memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
		..AllocationCreateInfo::default()
	};
	let buffer_info = BufferCreateInfo {
		usage: BufferUsage::STORAGE_BUFFER,
		..BufferCreateInfo::default()
	};

	let quads = (0..31)
		.flat_map(|x| (0..31).map(move |y| vec3(x as f32, y as f32, 0.)))
		.collect::<Vec<_>>();

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
						MeshletDrawVertex::new(quad),
						MeshletDrawVertex::new(quad + vec3(1., 0., 0.)),
						MeshletDrawVertex::new(quad + vec3(0., 1., 0.)),
						MeshletDrawVertex::new(quad + vec3(1., 1., 0.)),
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
				draw_vertex_offset: MeshletOffset::new(i * 4, 4),
				triangle_offset: MeshletOffset::new(i * 2, 2),
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
				draw_vertices: vertices.to_strong(),
				triangles: indices.to_strong(),
				meshlets: meshlets.to_strong(),
				num_meshlets: meshlets.len() as u32,
			},
		)
		.unwrap();

	let instances = init
		.bindless
		.buffer()
		.alloc_from_iter(
			init.memory_allocator.clone(),
			buffer_info.clone(),
			alloc_info.clone(),
			(0..1)
				.flat_map(|x| {
					(0..1).flat_map(move |y| {
						(0..4).map(move |z| {
							MeshletInstance::new(Affine3A::from_translation(vec3(
								x as f32 * 31.,
								y as f32 * 31.,
								z as f32 * 4.,
							)))
						})
					})
				})
				.collect::<Vec<_>>(),
		)
		.unwrap();

	MeshletMesh2InstanceCpu {
		mesh2instance: MeshletMesh2Instance {
			mesh: mesh.into(),
			instances: instances.into(),
		},
		num_meshlets: meshlets.len() as u32,
	}
}

impl MeshletRenderTask {
	pub fn new(init: &Arc<Init>, format_color: Format, format_depth: Format) -> Self {
		let pipeline_mesh = MeshDrawPipeline::new(init, format_color, format_depth);
		let mesh2instance = upload_test_mesh(init);

		Self {
			init: init.clone(),
			pipeline_mesh,
			mesh2instances: Vec::from([mesh2instance]),
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
		for mesh2instance in &self.mesh2instances {
			self.pipeline_mesh.draw(frame_context, &mut cmd, mesh2instance);
		}
		cmd.end_rendering().unwrap();
		let cmd = cmd.end().unwrap();

		future.then_execute(graphics.clone(), cmd).unwrap()
	}
}
