use crate::delta_time::DeltaTime;
use glam::{vec3, Mat3, Vec3};
use space_engine_shader::material::light::DirectionalLight;
use space_engine_shader::material::radiance::Radiance;
use space_engine_shader::renderer::lighting::sky_shader::preetham_sky;
use space_engine_shader::utils::animated_segments::{AnimatedSegment, Segment};
use std::f32::consts::PI;

const SUN_MAX_ALTITUDE_DEGREE: f32 = 25.;
const SUN_INCLINATION_SPEED: f32 = 0.5;
const SUN_INCLINATION_START: f32 = 1.7;
const SUN_INCLINATION_CURVE: AnimatedSegment<f32> = AnimatedSegment::new(&[
	Segment::new(0., 0.),
	Segment::new(0.2, 0.05),
	Segment::new(0.5, 0.1),
	Segment::new(1., 0.15),
	Segment::new(2., 0.2),
	Segment::new(3., 0.25),
	Segment::new(4., 0.3),
	Segment::new(4., 0.7),
	Segment::new(5., 0.75),
	Segment::new(6., 0.8),
	Segment::new(7., 0.85),
	Segment::new(8. - 0.5, 0.9),
	Segment::new(8. - 0.2, 0.95),
	Segment::new(8., 1.),
]);

pub fn eval_sun(delta_time: DeltaTime) -> DirectionalLight {
	let sun_dir = vec3(0., 1., 0.);
	let inclination =
		SUN_INCLINATION_CURVE.lerp(delta_time.since_start * SUN_INCLINATION_SPEED + SUN_INCLINATION_START);
	let sun_dir = Mat3::from_axis_angle(vec3(1., 0., 0.), inclination * 2. * PI) * sun_dir;
	let sun_dir = Mat3::from_axis_angle(vec3(0., 0., 1.), f32::to_radians(SUN_MAX_ALTITUDE_DEGREE)) * sun_dir;
	// not strictly necessary, but why not correct some inaccuracy?
	let sun_dir = sun_dir.normalize();

	let color = preetham_sky(sun_dir, sun_dir) / 1_000_000.;
	let color = color.clamp(Vec3::splat(0.), Vec3::splat(1.));
	DirectionalLight {
		direction: sun_dir,
		color: Radiance(color),
	}
}

pub fn eval_ambient_light(sun: DirectionalLight) -> Radiance {
	{
		const AMBIENT_STARLIGHT: Vec3 = vec3(105. / 255., 129. / 255., 142. / 255.);
		Radiance(sun.color.0 * 0.1 + AMBIENT_STARLIGHT * 0.1)
	}
}
