use glam::{vec2, vec3, vec3a};
use space_engine::space::renderer::model::gltf::load_gltf;
use space_engine::space::renderer::model::opaque::OpaqueModel;
use space_engine::space::Init;
use space_engine_shader::space::renderer::model::ModelVertex;
use std::sync::Arc;
use vulkano::image::ImageUsage;
use vulkano_bindless::descriptor::reference::StrongDesc;
use vulkano_bindless::spirv_std::image::Image2d;

pub async fn load_scene(init: &Arc<Init>) -> Vec<OpaqueModel> {
	let mut out = Vec::new();
	load_rust_vulkano_logos(init, &mut out).await;
	out.extend(
		load_gltf(
			init,
			concat!(
				env!("CARGO_MANIFEST_DIR"),
				"/src/sample_scene/Lantern/glTF/Lantern.gltf"
			),
		)
		.await,
	);
	out
}

pub async fn load_rust_vulkano_logos(init: &Arc<Init>, out: &mut Vec<OpaqueModel>) {
	let create_model = |texture: StrongDesc<Image2d>| {
		let vertices = [
			ModelVertex::new(vec3(-1., -1., 0.), vec2(0., 0.), texture),
			ModelVertex::new(vec3(-1., 1., 0.), vec2(0., 1.), texture),
			ModelVertex::new(vec3(1., -1., 0.), vec2(1., 0.), texture),
			ModelVertex::new(vec3(1., 1., 0.), vec2(1., 1.), texture),
		];
		let indices = [0, 1, 2, 1, 2, 3];
		(vertices, indices)
	};

	let vulkano_tex = OpaqueModel::upload_texture(
		init,
		ImageUsage::SAMPLED,
		image::load_from_memory(include_bytes!("vulkano_logo.png")).unwrap(),
	);
	let rust_mascot_tex = OpaqueModel::upload_texture(
		init,
		ImageUsage::SAMPLED,
		image::load_from_memory(include_bytes!("rust_mascot.png")).unwrap(),
	);
	let vulkano_tex = vulkano_tex.await;
	let rust_mascot_tex = rust_mascot_tex.await;

	// unroll indices
	let (vertices, indices) = create_model(vulkano_tex.to_strong());
	let vulkano_logo = OpaqueModel::direct(init, indices.map(|i| vertices[i as usize]).into_iter());

	// use indices
	let (vertices, indices) = create_model(rust_mascot_tex.to_strong());
	let rust_mascot = OpaqueModel::indexed(
		init,
		indices,
		vertices.map(|v| ModelVertex {
			position: v.position + vec3a(0., 0.5, 1.),
			..v
		}),
	);
	out.extend([vulkano_logo, rust_mascot])
}
