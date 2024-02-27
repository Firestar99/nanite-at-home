use std::sync::Arc;

use glam::{vec2, vec3, vec3a};

use space_engine::space::renderer::model::model::OpaqueModel;
use space_engine::space::renderer::model::model_descriptor_set::ModelDescriptorSetLayout;
use space_engine::space::renderer::model::model_gltf::load_gltf;
use space_engine::space::renderer::model::texture_manager::TextureManager;
use space_engine::space::Init;
use space_engine_common::space::renderer::model::model_vertex::{ModelTextureId, ModelVertex};

pub async fn load_scene(init: &Arc<Init>, texture_manager: &Arc<TextureManager>) -> Vec<OpaqueModel> {
	let model_descriptor_set_layout = ModelDescriptorSetLayout::new(init);
	let mut out = Vec::new();
	load_rust_vulkano_logos(&init, &texture_manager, &model_descriptor_set_layout, &mut out).await;
	out.extend(
		load_gltf(
			&init,
			&texture_manager,
			&model_descriptor_set_layout,
			"", // FIXME model path
		)
		.await,
	);
	out
}

pub async fn load_rust_vulkano_logos(
	init: &Arc<Init>,
	texture_manager: &Arc<TextureManager>,
	model_descriptor_set_layout: &ModelDescriptorSetLayout,
	out: &mut Vec<OpaqueModel>,
) {
	const QUAD_VERTICES: [ModelVertex; 4] = [
		ModelVertex::new(vec3(-1., -1., 0.), vec2(0., 0.), ModelTextureId(0)),
		ModelVertex::new(vec3(-1., 1., 0.), vec2(0., 1.), ModelTextureId(0)),
		ModelVertex::new(vec3(1., -1., 0.), vec2(1., 0.), ModelTextureId(0)),
		ModelVertex::new(vec3(1., 1., 0.), vec2(1., 1.), ModelTextureId(0)),
	];
	const QUAD_INDICES: [u32; 6] = [0, 1, 2, 1, 2, 3];

	// unroll indices
	let (_, vulkano_tex_id) = texture_manager
		.upload_texture_from_memory(include_bytes!("vulkano_logo.png"))
		.await
		.unwrap();
	let vulkano_logo = OpaqueModel::direct(
		init,
		texture_manager,
		&model_descriptor_set_layout,
		QUAD_INDICES
			.map(|i| QUAD_VERTICES[i as usize])
			.map(|v| ModelVertex {
				tex_id: vulkano_tex_id,
				..v
			})
			.into_iter(),
	);

	// use indices
	let (_, rust_mascot_tex_id) = texture_manager
		.upload_texture_from_memory(include_bytes!("rust_mascot.png"))
		.await
		.unwrap();
	let rust_mascot = OpaqueModel::indexed(
		init,
		texture_manager,
		&model_descriptor_set_layout,
		QUAD_INDICES.iter().copied(),
		QUAD_VERTICES.iter().copied().map(|v| ModelVertex {
			position: v.position + vec3a(0., 0.5, 1.),
			tex_id: rust_mascot_tex_id,
			..v
		}),
	);

	let vulkano_logo = vulkano_logo.await;
	let rust_mascot = rust_mascot.await;
	out.extend([vulkano_logo, rust_mascot])
}
