use glam::{vec2, vec3, vec3a};
use space_engine::space::renderer::model::model::OpaqueModel;
use space_engine::space::renderer::model::model_descriptor_set::ModelDescriptorSetLayout;
use space_engine::space::renderer::model::texture_manager::TextureManager;
use space_engine::space::Init;
use space_engine_common::space::renderer::model::model_vertex::ModelVertex;
use std::sync::Arc;

pub async fn load_scene(init: &Arc<Init>, texture_manager: &Arc<TextureManager>) -> Vec<OpaqueModel> {
	let model_descriptor_set_layout = ModelDescriptorSetLayout::new(init);

	const QUAD_VERTICES: [ModelVertex; 4] = [
		ModelVertex::new(vec3(-1., -1., 0.), vec2(0., 0.)),
		ModelVertex::new(vec3(-1., 1., 0.), vec2(0., 1.)),
		ModelVertex::new(vec3(1., -1., 0.), vec2(1., 0.)),
		ModelVertex::new(vec3(1., 1., 0.), vec2(1., 1.)),
	];
	const QUAD_INDICES: [u16; 6] = [0, 1, 2, 1, 2, 3];

	// unroll indices
	let vulkano_logo = OpaqueModel::direct(
		init,
		texture_manager,
		&model_descriptor_set_layout,
		QUAD_INDICES.map(|i| QUAD_VERTICES[i as usize]).into_iter(),
		include_bytes!("vulkano_logo.png"),
	);

	// use indices
	let rust_mascot = OpaqueModel::indexed(
		init,
		texture_manager,
		&model_descriptor_set_layout,
		QUAD_INDICES.iter().copied(),
		QUAD_VERTICES.iter().copied().map(|v| ModelVertex {
			position: v.position + vec3a(0., 0.5, 1.),
			..v
		}),
		include_bytes!("rust_mascot.png"),
	);

	let vulkano_logo = vulkano_logo.await;
	let rust_mascot = rust_mascot.await;
	Vec::from([vulkano_logo, rust_mascot])
}
