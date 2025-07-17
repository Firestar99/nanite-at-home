#![cfg(test)]

use crate::debugger;
use ash::vk::{
	ColorComponentFlags, CullModeFlags, FrontFace, PipelineColorBlendAttachmentState,
	PipelineColorBlendStateCreateInfo, PolygonMode, PrimitiveTopology,
};
use glam::{UVec2, Vec2, Vec4};
use integration_test_shader::color::ColorEnum;
use integration_test_shader::triangle::{Param, Vertex};
use pollster::block_on;
use rust_gpu_bindless_core::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessImageCreateInfo,
	BindlessImageUsage, BindlessInstance, DescBufferLenExt, DescriptorCounts, Extent, Format, Image2d,
	MutDescBufferExt, RCDescExt,
};
use rust_gpu_bindless_core::pipeline::{
	ClearValue, ColorAttachment, DrawIndirectCommand, GraphicsPipelineCreateInfo, HostAccess, LoadOp,
	MutBufferAccessExt, MutImageAccessExt, PipelineDepthStencilStateCreateInfo, PipelineInputAssemblyStateCreateInfo,
	PipelineRasterizationStateCreateInfo, RenderPassFormat, RenderingAttachment, StoreOp, TransferRead, TransferWrite,
};
use rust_gpu_bindless_core::platform::BindlessPipelinePlatform;
use rust_gpu_bindless_core::platform::ash::{Ash, AshSingleGraphicsQueueCreateInfo, ash_init_single_graphics_queue};
use smallvec::SmallVec;

const R: ColorEnum = ColorEnum::Red;
const C: ColorEnum = ColorEnum::Cyan;
const B: ColorEnum = ColorEnum::Black;
const Y: ColorEnum = ColorEnum::Yellow;

#[test]
fn test_triangle_ash() -> anyhow::Result<()> {
	unsafe {
		let bindless = BindlessInstance::<Ash>::new(
			ash_init_single_graphics_queue(AshSingleGraphicsQueueCreateInfo {
				debug: debugger(),
				..AshSingleGraphicsQueueCreateInfo::default()
			})?,
			DescriptorCounts::REASONABLE_DEFAULTS,
		);
		block_on(test_triangle(&bindless))?;
		Ok(())
	}
}

async fn test_triangle<P: BindlessPipelinePlatform>(bindless: &Bindless<P>) -> anyhow::Result<()> {
	let vertices = bindless.buffer().alloc_shared_from_iter(
		&BindlessBufferCreateInfo {
			name: "vertices",
			usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::STORAGE_BUFFER,
			allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
		},
		[
			Vertex::new(Vec2::new(-1., -1.), R.color()),
			Vertex::new(Vec2::new(1., -1.), R.color()),
			Vertex::new(Vec2::new(-1., 1.), R.color()),
			Vertex::new(Vec2::new(1., -1.), C.color()),
			Vertex::new(Vec2::new(1., 1.), C.color()),
			Vertex::new(Vec2::new(-1., -1.), C.color()),
			// yellow triangle is backface culled
			Vertex::new(Vec2::new(1., -1.), Y.color()),
			Vertex::new(Vec2::new(-1., -1.), Y.color()),
			Vertex::new(Vec2::new(1., 1.), Y.color()),
		],
	)?;

	let rt_format = Format::R8G8B8A8_UNORM;
	let render_pass_format = RenderPassFormat {
		color_attachments: SmallVec::from_slice(&[rt_format]),
		depth_attachment: None,
	};

	let pipeline = bindless.create_graphics_pipeline(
		&render_pass_format,
		&GraphicsPipelineCreateInfo {
			input_assembly_state: PipelineInputAssemblyStateCreateInfo::default()
				.topology(PrimitiveTopology::TRIANGLE_LIST),
			rasterization_state: PipelineRasterizationStateCreateInfo::default()
				.polygon_mode(PolygonMode::FILL)
				.front_face(FrontFace::CLOCKWISE)
				.cull_mode(CullModeFlags::BACK),
			depth_stencil_state: PipelineDepthStencilStateCreateInfo::default(),
			color_blend_state: PipelineColorBlendStateCreateInfo::default().attachments(&[
				PipelineColorBlendAttachmentState::default().color_write_mask(ColorComponentFlags::RGBA),
			]),
		},
		crate::shader::triangle::triangle_vertex::new(),
		crate::shader::triangle::triangle_fragment::new(),
	)?;

	let rt_extent = UVec2::new(8, 8);
	let rt_image = bindless.image().alloc::<Image2d>(&BindlessImageCreateInfo {
		name: "rt",
		format: rt_format,
		extent: Extent::from(rt_extent),
		usage: BindlessImageUsage::TRANSFER_SRC | BindlessImageUsage::COLOR_ATTACHMENT,
		..BindlessImageCreateInfo::default()
	})?;

	let rt_size = (rt_extent.x * rt_extent.y) as usize;
	let rt_download = bindless.buffer().alloc_slice::<[u8; 4]>(
		&BindlessBufferCreateInfo {
			name: "staging_upload",
			usage: BindlessBufferUsage::MAP_READ | BindlessBufferUsage::TRANSFER_DST,
			allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
		},
		rt_size,
	)?;

	let rt_download = bindless.execute(|cmd| {
		let rt_download = rt_download.access::<TransferWrite>(cmd)?;
		let mut image = rt_image.access::<ColorAttachment>(cmd)?;
		cmd.begin_rendering(
			render_pass_format,
			&[RenderingAttachment {
				image: &mut image,
				load_op: LoadOp::Clear(ClearValue::ColorF(B.color().to_array())),
				store_op: StoreOp::Store,
			}],
			None,
			|rp| {
				rp.draw(
					&pipeline,
					DrawIndirectCommand {
						vertex_count: vertices.len() as u32,
						instance_count: 1,
						first_vertex: 0,
						first_instance: 0,
					},
					Param {
						vertices: vertices.to_transient(rp),
					},
				)?;
				Ok(())
			},
		)?;

		let image = image.transition::<TransferRead>()?;
		unsafe { cmd.copy_image_to_buffer(&image, &rt_download)? };

		Ok(rt_download.transition::<HostAccess>()?.into_desc())
	})?;

	// 5. downloads the data from `staging_download` and verify the contents
	let result = rt_download
		.mapped()
		.await?
		.read_iter()
		.map(|c| ColorEnum::parse(Vec4::from_array(c.map(|v| v as f32)) / 255.))
		.collect::<Vec<_>>();
	let result = result.chunks_exact(rt_extent.x as usize).collect::<Vec<_>>();
	assert_eq!(
		&*result,
		&[
			[C, C, C, C, C, C, C, C],
			[R, C, C, C, C, C, C, C],
			[R, R, C, C, C, C, C, C],
			[R, R, R, C, C, C, C, C],
			[R, R, R, B, C, C, C, C],
			[R, R, B, B, B, C, C, C],
			[R, B, B, B, B, B, C, C],
			[B, B, B, B, B, B, B, C]
		]
	);
	Ok(())
}
