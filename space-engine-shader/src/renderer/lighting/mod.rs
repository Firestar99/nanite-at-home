pub mod lighting_compute;
pub mod sky_shader;

pub fn is_skybox(albedo_alpha: f32) -> bool {
	albedo_alpha < 0.01
}
