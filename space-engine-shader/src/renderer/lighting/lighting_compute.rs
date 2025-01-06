use crate::material::pbr::{SampledMaterial, V};
use crate::material::radiance::Radiance;
use crate::renderer::camera::Camera;
use crate::renderer::frame_data::{DebugSettings, FrameData};
use crate::renderer::g_buffer::GBuffer;
use crate::renderer::lighting::is_skybox;
use crate::utils::hsv::hsv2rgb_smooth;
use crate::utils::srgb::linear_to_srgb_alpha;
use glam::{uvec2, vec3, UVec2, UVec3, Vec3, Vec3Swizzles, Vec4, Vec4Swizzles};
use rust_gpu_bindless_macros::{bindless, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{
	AliveDescRef, Buffer, Descriptors, Image2d, MutImage, Transient, TransientDesc,
};
use static_assertions::const_assert_eq;

#[derive(Copy, Clone, BufferStruct)]
pub struct Param<'a> {
	pub frame_data: TransientDesc<'a, Buffer<FrameData>>,
	pub g_buffer: GBuffer<Transient<'a>>,
	pub output_image: TransientDesc<'a, MutImage<Image2d>>,
}

pub const LIGHTING_WG_SIZE: u32 = 64;

const_assert_eq!(LIGHTING_WG_SIZE, 64);
#[bindless(compute(threads(64)))]
pub fn lighting_cs(
	#[bindless(descriptors)] descriptors: Descriptors,
	#[bindless(param)] param: &Param<'static>,
	#[spirv(workgroup_id)] wg_id: UVec3,
	#[spirv(local_invocation_id)] inv_id: UVec3,
) {
	let frame_data = param.frame_data.access(&descriptors).load();
	let size: UVec2 = frame_data.viewport_size;
	let pixel_wg_start = wg_id.xy() * uvec2(64, 1);
	let pixel = pixel_wg_start + uvec2(inv_id.x, 0);
	let pixel_inbounds = pixel.x < size.x && pixel.y < size.y;

	let (sampled, debug_hue) =
		sampled_material_from_g_buffer(frame_data.camera, &descriptors, param.g_buffer, pixel, size);
	let skybox = is_skybox(sampled.alpha);

	let out_color = match frame_data.debug_settings() {
		DebugSettings::None => material_eval(frame_data, sampled),
		DebugSettings::MeshletIdOverlay | DebugSettings::TriangleIdOverlay => {
			Vec3::lerp(material_eval(frame_data, sampled), debug_color(debug_hue), 0.1)
		}
		DebugSettings::MeshletId | DebugSettings::TriangleId | DebugSettings::LodLevel => debug_color(debug_hue),
		DebugSettings::BaseColor => sampled.albedo,
		DebugSettings::Normals | DebugSettings::VertexNormals => sampled.normal,
		DebugSettings::RoughnessMetallic => vec3(0., sampled.roughness, sampled.metallic),
		DebugSettings::ReconstructedPosition => {
			if sampled.alpha < 0.001 {
				Vec3::ZERO
			} else {
				let depth = Vec4::from(param.g_buffer.depth_image.access(&descriptors).fetch(pixel)).x;
				let position = frame_data
					.camera
					.reconstruct_from_depth(pixel.as_vec2() / size.as_vec2(), depth);

				let ipos = (position.world_space.xyz() * 10.).as_ivec3();
				if (ipos.x & 1 == 0) ^ (ipos.y & 1 == 0) ^ (ipos.z & 1 == 0) {
					sampled.albedo
				} else {
					vec3(0., 0., 0.)
				}
			}
		}
	};

	let out_color = Vec4::from((out_color, 1.));
	let out_color = linear_to_srgb_alpha(out_color);
	if pixel_inbounds && !skybox {
		unsafe {
			param.output_image.access(&descriptors).write(pixel, out_color);
		}
	}
}

#[allow(clippy::useless_conversion)]
fn sampled_material_from_g_buffer(
	camera: Camera,
	descriptors: &Descriptors,
	g_buffer: GBuffer<impl AliveDescRef>,
	pixel: UVec2,
	size: UVec2,
) -> (SampledMaterial, f32) {
	let albedo = Vec4::from(g_buffer.g_albedo.access(descriptors).fetch(pixel));
	let alpha = albedo.w;
	let albedo = albedo.xyz();
	let normal = Vec4::from(g_buffer.g_normal.access(descriptors).fetch(pixel));
	let meshlet_debug_hue = normal.w;
	let normal = normal.xyz() * 2. - 1.;
	let [roughness, metallic] = Vec4::from(g_buffer.g_roughness_metallic.access(descriptors).fetch(pixel))
		.xy()
		.to_array();
	let depth = Vec4::from(g_buffer.depth_image.access(descriptors).fetch(pixel)).x;

	let position = camera.reconstruct_from_depth(pixel.as_vec2() / size.as_vec2(), depth);
	let sampled = SampledMaterial {
		world_pos: position.world_space,
		v: V::new(position.world_space, camera.transform.translation()),
		albedo,
		alpha,
		normal,
		roughness,
		metallic,
	};
	(sampled, meshlet_debug_hue)
}

fn material_eval(frame_data: FrameData, sampled: SampledMaterial) -> Vec3 {
	let mut lo = Radiance(Vec3::ZERO);
	lo += sampled.evaluate_directional_light(frame_data.sun);
	lo += sampled.ambient_light(frame_data.ambient_light);
	lo.tone_map_reinhard()
}

fn debug_color(meshlet_debug_hue: f32) -> Vec3 {
	if meshlet_debug_hue < 0.0001 {
		vec3(0., 0., 0.)
	} else {
		hsv2rgb_smooth(vec3(meshlet_debug_hue, 1., 1.))
	}
}
