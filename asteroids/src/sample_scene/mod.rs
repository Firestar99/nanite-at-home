use std::sync::Arc;

use glam::{vec2, vec3, vec3a};

use space_engine::space::renderer::model::model::OpaqueModel;
use space_engine::space::renderer::model::model_gltf::load_gltf;
use space_engine::space::renderer::model::texture_manager::TextureManager;
use space_engine_common::space::renderer::model::model_vertex::ModelVertex;
use vulkano_bindless::descriptor::{SampledImage2D, WeakDesc};

pub async fn load_scene(texture_manager: &Arc<TextureManager>) -> Vec<OpaqueModel> {
	let mut out = Vec::new();
	load_rust_vulkano_logos(&texture_manager, &mut out).await;
	out.extend(
		load_gltf(
			&texture_manager,
			concat!(
				env!("CARGO_MANIFEST_DIR"),
				"/src/sample_scene/Lantern/glTF/Lantern.gltf"
			),
		)
		.await,
	);
	out
}

pub async fn load_rust_vulkano_logos(texture_manager: &Arc<TextureManager>, out: &mut Vec<OpaqueModel>) {
	let create_model = |texture: WeakDesc<SampledImage2D>| {
		let vertices = [
			ModelVertex::new(vec3(-1., -1., 0.), vec2(0., 0.), texture),
			ModelVertex::new(vec3(-1., 1., 0.), vec2(0., 1.), texture),
			ModelVertex::new(vec3(1., -1., 0.), vec2(1., 0.), texture),
			ModelVertex::new(vec3(1., 1., 0.), vec2(1., 1.), texture),
		];
		let indices = [0, 1, 2, 1, 2, 3];
		(vertices, indices)
	};

	let vulkano_tex = texture_manager.upload_texture_from_memory(include_bytes!("vulkano_logo.png"));
	let rust_mascot_tex = texture_manager.upload_texture_from_memory(include_bytes!("rust_mascot.png"));
	let vulkano_tex = vulkano_tex.await.unwrap();
	let rust_mascot_tex = rust_mascot_tex.await.unwrap();

	// unroll indices
	let (vertices, indices) = create_model(vulkano_tex.to_weak());
	let vulkano_logo = OpaqueModel::direct(
		texture_manager,
		indices.map(|i| vertices[i as usize]).into_iter(),
		[vulkano_tex],
	);

	// use indices
	let (vertices, indices) = create_model(rust_mascot_tex.to_weak());
	let rust_mascot = OpaqueModel::indexed(
		texture_manager,
		indices,
		vertices.map(|v| ModelVertex {
			position: v.position + vec3a(0., 0.5, 1.),
			..v
		}),
		[rust_mascot_tex],
	);

	let vulkano_logo = vulkano_logo.await;
	let rust_mascot = rust_mascot.await;
	out.extend([vulkano_logo, rust_mascot])
}
