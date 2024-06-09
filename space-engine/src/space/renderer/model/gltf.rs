use crate::space::renderer::model::opaque::OpaqueModelCpu;
use crate::space::renderer::model::OpaqueVertex;
use crate::space::Init;
use futures::future::join_all;
use glam::{vec4, Mat4, Vec2, Vec3};
use gltf::image::{Data, Format};
use gltf::mesh::Mode;
use gltf::{Document, Node, Scene};
use image::DynamicImage;
use std::path::Path;
use std::sync::Arc;
use vulkano::image::ImageUsage;
use vulkano_bindless::descriptor::RCDescExt;

pub async fn load_gltf(init: &Arc<Init>, path: impl AsRef<Path>) -> Vec<OpaqueModelCpu> {
	load_gltf_inner(init, path.as_ref()).await
}

async fn load_gltf_inner(init: &Arc<Init>, path: &Path) -> Vec<OpaqueModelCpu> {
	let (document, buffers, images) = gltf::import(path).unwrap();

	let scene = document.default_scene().unwrap();
	let nodes_mat = compute_transformations(
		&document,
		&scene,
		Mat4 {
			y_axis: vec4(0., -1., 0., 0.),
			..Mat4::IDENTITY
		},
	);

	let white_image = OpaqueModelCpu::upload_texture(
		init,
		ImageUsage::SAMPLED,
		gltf_image_to_dynamic_image(Data {
			format: Format::R8G8B8A8,
			width: 1,
			height: 1,
			pixels: Vec::from([0xffu8; 4]),
		}),
	)
	.await;
	let images = join_all(
		images
			.into_iter()
			.map(|src| OpaqueModelCpu::upload_texture(init, ImageUsage::SAMPLED, gltf_image_to_dynamic_image(src))),
	)
	.await;

	let mut models = Vec::new();
	for node in document.nodes() {
		let mat = nodes_mat[node.index()];
		if let Some(mesh) = node.mesh() {
			for primitive in mesh.primitives() {
				assert_eq!(primitive.mode(), Mode::Triangles);
				let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

				let albedo_tex = primitive
					.material()
					.pbr_metallic_roughness()
					.base_color_texture()
					.map(|tex| &images[tex.texture().source().index()])
					.unwrap_or_else(|| &white_image);
				let desc = albedo_tex.to_strong();
				let model_vertices = reader
					.read_positions()
					.unwrap()
					.zip(reader.read_tex_coords(0).unwrap().into_f32())
					.map(|(pos, tex_coord)| {
						OpaqueVertex::new(mat.transform_point3(Vec3::from(pos)), Vec2::from(tex_coord), desc)
					});

				if let Some(model_indices) = reader.read_indices() {
					models.push(OpaqueModelCpu::indexed(init, model_indices.into_u32(), model_vertices));
				} else {
					models.push(OpaqueModelCpu::direct(init, model_vertices));
				}
			}
		}
	}
	models
}

fn gltf_image_to_dynamic_image(src: Data) -> DynamicImage {
	match src.format {
		Format::R8 => DynamicImage::ImageLuma8(
			image::ImageBuffer::<image::Luma<u8>, _>::from_raw(src.width, src.height, src.pixels).unwrap(),
		),
		Format::R8G8 => DynamicImage::ImageLumaA8(
			image::ImageBuffer::<image::LumaA<u8>, _>::from_raw(src.width, src.height, src.pixels).unwrap(),
		),
		Format::R8G8B8 => DynamicImage::ImageRgb8(
			image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(src.width, src.height, src.pixels).unwrap(),
		),
		Format::R8G8B8A8 => DynamicImage::ImageRgba8(
			image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(src.width, src.height, src.pixels).unwrap(),
		),
		e => panic!("unsupported image format: {:?}", e),
	}
}

fn compute_transformations(document: &Document, scene: &Scene, base: Mat4) -> Vec<Mat4> {
	fn walk(out: &mut Vec<Mat4>, node: Node, mat: Mat4) {
		let node_mat = mat * Mat4::from_cols_array_2d(&node.transform().matrix());
		out[node.index()] = node_mat;
		for node in node.children() {
			walk(out, node, node_mat);
		}
	}

	let mut out = vec![Mat4::IDENTITY; document.nodes().len()];
	for node in scene.nodes() {
		walk(&mut out, node, base);
	}
	out
}
