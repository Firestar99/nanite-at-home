use std::path::Path;
use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};
use gltf::image::Format;
use gltf::mesh::Mode;
use gltf::{Document, Node, Scene};
use image::DynamicImage;

use space_engine_common::space::renderer::model::model_vertex::ModelVertex;

use crate::space::renderer::model::model::OpaqueModel;
use crate::space::renderer::model::model_descriptor_set::ModelDescriptorSetLayout;
use crate::space::renderer::model::texture_manager::TextureManager;
use crate::space::Init;

pub async fn load_gltf(
	init: &Arc<Init>,
	texture_manager: &Arc<TextureManager>,
	model_descriptor_set_layout: &ModelDescriptorSetLayout,
	path: impl AsRef<Path>,
) -> Vec<OpaqueModel> {
	load_gltf_inner(init, texture_manager, model_descriptor_set_layout, path.as_ref()).await
}

async fn load_gltf_inner(
	init: &Arc<Init>,
	texture_manager: &Arc<TextureManager>,
	model_descriptor_set_layout: &ModelDescriptorSetLayout,
	path: &Path,
) -> Vec<OpaqueModel> {
	let (document, buffers, images) = gltf::import(path).unwrap();

	let scene = document.default_scene().unwrap();
	let nodes = compute_transformations(&document, &scene);

	let images = futures::future::join_all(images.into_iter().map(|src| {
		let image = match src.format {
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
		};
		texture_manager.upload_texture(image)
	}))
	.await;

	let mut models = Vec::new();
	for node in document.nodes() {
		let mat = nodes[node.index()];
		if let Some(mesh) = node.mesh() {
			for primitive in mesh.primitives() {
				assert_eq!(primitive.mode(), Mode::Triangles);
				let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
				let vertices = reader
					.read_positions()
					.unwrap()
					.zip(reader.read_tex_coords(0).unwrap().into_f32())
					.map(|(pos, tex_coord)| {
						ModelVertex::new(mat.transform_point3(Vec3::from(pos)), Vec2::from(tex_coord))
					});
				let indices = reader.read_indices().map(|v| v.into_u32().map(|i| i as u16));
				let albedo_tex_id = images[primitive
					.material()
					.pbr_metallic_roughness()
					.base_color_texture()
					.unwrap()
					.texture()
					.source()
					.index()]
				.1;
				models.push(if let Some(indices) = indices {
					OpaqueModel::indexed(
						init,
						texture_manager,
						model_descriptor_set_layout,
						indices,
						vertices,
						albedo_tex_id,
					)
					.await
				} else {
					OpaqueModel::direct(
						init,
						texture_manager,
						model_descriptor_set_layout,
						vertices,
						albedo_tex_id,
					)
					.await
				});
			}
		}
	}
	models
}

fn compute_transformations(document: &Document, scene: &Scene) -> Vec<Mat4> {
	fn walk(out: &mut Vec<Mat4>, node: Node, mat: Mat4) {
		let node_mat = mat * Mat4::from_cols_array_2d(&node.transform().matrix());
		out[node.index()] = node_mat;
		for node in node.children() {
			walk(out, node, node_mat);
		}
	}

	let mut out = vec![Mat4::IDENTITY; document.nodes().len()];
	for node in scene.nodes() {
		walk(&mut out, node, Mat4::IDENTITY);
	}
	out
}
