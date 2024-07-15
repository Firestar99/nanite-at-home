use crate::delta_time::DeltaTime;
use glam::{vec3, Affine3A, DVec2, Quat, Vec3};
use num_traits::clamp;
use std::f32;
use std::f32::consts::PI;
use std::ops::{Deref, DerefMut};
use winit::dpi::PhysicalPosition;
use winit::event::ElementState::Pressed;
use winit::event::{DeviceEvent, Event, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::keyboard::PhysicalKey::Code;

#[derive(Copy, Clone)]
pub struct FpsCameraController {
	pub state: State,
	pub move_speed: Vec3,
	pub mouse_speed: f32,
}

#[derive(Copy, Clone, Default)]
pub struct State {
	pub position: Vec3,
	pub rotation_yaw: f32,
	pub rotation_pitch: f32,
	/// should not be pub: used for remembering key states
	movement_keys: [[bool; 2]; 3],
	pub move_speed_exponent: i32,
}

impl Deref for FpsCameraController {
	type Target = State;

	fn deref(&self) -> &Self::Target {
		&self.state
	}
}

impl DerefMut for FpsCameraController {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.state
	}
}

impl Default for FpsCameraController {
	fn default() -> Self {
		Self {
			state: State::default(),
			move_speed: Vec3::splat(1.),
			mouse_speed: 0.02,
		}
	}
}

impl FpsCameraController {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn handle_input(&mut self, event: &Event<()>) {
		match event {
			Event::WindowEvent {
				event:
					WindowEvent::KeyboardInput {
						event: KeyEvent {
							state,
							physical_key: Code { 0: code },
							..
						},
						..
					},
				..
			} => {
				use winit::keyboard::KeyCode::*;
				let value = *state == Pressed;
				match code {
					KeyA => self.movement_keys[0][0] = value,
					KeyD => self.movement_keys[0][1] = value,
					Space => self.movement_keys[1][0] = value,
					ShiftLeft => self.movement_keys[1][1] = value,
					KeyW => self.movement_keys[2][0] = value,
					KeyS => self.movement_keys[2][1] = value,
					Home => self.state = State::default(),
					_ => {}
				}
			}
			Event::DeviceEvent {
				event: DeviceEvent::MouseMotion { delta, .. },
				..
			} => {
				const MOUSE_SPEED_CONST: f32 = 1. / (2. * PI);
				let delta = DVec2::from(*delta).as_vec2() * self.mouse_speed * MOUSE_SPEED_CONST;
				self.rotation_yaw -= delta.x;
				self.rotation_pitch = clamp(self.rotation_pitch + delta.y, -PI / 2., PI / 2.);
			}
			Event::WindowEvent {
				event: WindowEvent::MouseWheel { delta, .. },
				..
			} => {
				let y = match *delta {
					MouseScrollDelta::PixelDelta(PhysicalPosition { y, .. }) => y as f32,
					MouseScrollDelta::LineDelta(_, y) => y,
				};
				let y = -y;
				self.move_speed_exponent += [-1, 1][(y < 0.) as usize];
			}
			_ => {}
		}
	}

	pub fn update(&mut self, delta_time: DeltaTime) -> Affine3A {
		let mut movement = Vec3::default();
		for dir in 0..3 {
			for ud in [0, 1] {
				movement[dir] += [0., [-1., 1.][ud]][usize::from(self.movement_keys[dir][ud])];
			}
		}
		let exponent_speed = f32::powf(2., self.move_speed_exponent as f32);
		movement *= self.move_speed * exponent_speed * *delta_time;

		let quat_yaw = Quat::from_axis_angle(vec3(0., 1., 0.), self.rotation_yaw);
		self.position += quat_yaw * movement;
		let quat = quat_yaw * Quat::from_axis_angle(vec3(1., 0., 0.), self.rotation_pitch);
		// Affine3A::from_translation(self.position) * Affine3A::from_quat(quat)
		Affine3A::from_quat(quat.conjugate()) * Affine3A::from_translation(-self.position)
	}
}
