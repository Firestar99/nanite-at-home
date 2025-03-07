use crate::delta_time::DeltaTime;
use egui::{Slider, SliderClamping, Ui, Widget};
use glam::{vec3, Mat3, Vec3};
use space_engine_shader::material::light::DirectionalLight;
use space_engine_shader::material::radiance::Radiance;
use space_engine_shader::renderer::lighting::sky_shader::preetham_sky;
use space_engine_shader::utils::animated_segments::{AnimatedSegment, Segment};
use std::f32::consts::PI;

const SUN_INCLINATION_DEGREE_DEFAULT: f32 = 25. + 90.;
const SUN_POSITION_SPEED: f32 = 0.5;
const SUN_POSITION_CURVE: AnimatedSegment<f32> = AnimatedSegment::new(&[
	Segment::new(0., 0.),
	Segment::new(0.2, 0.05),
	Segment::new(0.5, 0.1),
	Segment::new(1., 0.15),
	Segment::new(2., 0.2),
	Segment::new(3.5, 0.275),
	Segment::new(3.5, 0.725),
	Segment::new(4., 0.75),
	Segment::new(5., 0.8),
	Segment::new(6., 0.85),
	Segment::new(7. - 0.5, 0.9),
	Segment::new(7. - 0.2, 0.95),
	Segment::new(7., 1.),
]);
const SUN_PERIOD_DEFAULT: f32 = SUN_POSITION_CURVE.max_time() / SUN_POSITION_SPEED;

pub struct SunController {
	pub is_paused: bool,
	pub position_dragged: bool,
	pub position: f32,
	pub period: f32,
	pub inclination_degree: f32,
}

impl SunController {
	pub fn new() -> Self {
		Self {
			is_paused: false,
			position_dragged: false,
			position: 0.,
			period: SUN_PERIOD_DEFAULT,
			inclination_degree: SUN_INCLINATION_DEGREE_DEFAULT,
		}
	}

	pub fn eval_sun(&mut self, delta_time: DeltaTime) -> (DirectionalLight, Radiance) {
		if !self.is_paused && !self.position_dragged {
			let speed = if self.period.abs() < 0.001 {
				0.
			} else {
				self.period.recip()
			};
			self.position = (self.position + speed * *delta_time).fract();
		}

		let sun_dir = vec3(0., 1., 0.);
		let position = SUN_POSITION_CURVE.lerp(self.position * SUN_POSITION_CURVE.max_time());
		let sun_dir = Mat3::from_axis_angle(vec3(1., 0., 0.), position * 2. * PI) * sun_dir;
		let inclination_radians = f32::to_radians(self.inclination_degree - 90.);
		let sun_dir = Mat3::from_axis_angle(vec3(0., 0., 1.), inclination_radians) * sun_dir;
		// not strictly necessary, but why not correct some inaccuracy?
		let sun_dir = sun_dir.normalize();
		// there's no science in this, just looks good
		let color = preetham_sky(sun_dir, sun_dir) / 1_000_000.;
		let color = color.clamp(Vec3::splat(0.), Vec3::splat(1.));
		let sun = DirectionalLight {
			direction: sun_dir,
			color: Radiance(color),
		};
		const AMBIENT_STARLIGHT: Vec3 = vec3(105. / 255., 129. / 255., 142. / 255.);
		let ambient = Radiance(sun.color.0 * 0.1 + AMBIENT_STARLIGHT * 0.1);
		(sun, ambient)
	}

	pub fn ui(&mut self, ui: &mut Ui) {
		ui.strong("Sun:");
		ui.checkbox(&mut self.is_paused, "Paused");
		let response = Slider::new(&mut self.position, 0.1..=1.)
			.clamping(SliderClamping::Never)
			.text("position")
			.ui(ui);
		self.position_dragged = response.is_pointer_button_down_on();
		Slider::new(&mut self.period, 1. ..=60.)
			.clamping(SliderClamping::Never)
			.suffix("s")
			.text("period")
			.ui(ui);
		Slider::new(&mut self.inclination_degree, 0. ..=180.)
			.text("inclination")
			.suffix("°")
			.ui(ui);
		if ui.button("reset sun").clicked() {
			self.position = 0.;
			self.period = SUN_PERIOD_DEFAULT;
			self.inclination_degree = SUN_INCLINATION_DEGREE_DEFAULT;
		}
	}
}
