use space_asset::meshlet::scene::{MeshletSceneCpu, MeshletSceneFile};
use space_asset::uploader::Uploader;
use space_engine::renderer::Init;
use std::io;
use std::sync::Arc;
use winit::event::ElementState::Pressed;
use winit::event::{Event, KeyEvent, WindowEvent};
use winit::keyboard::PhysicalKey::Code;

pub struct SceneSelector<'a, F>
where
	F: FnMut(Arc<MeshletSceneCpu>),
{
	init: Arc<Init>,
	scenes: Vec<MeshletSceneFile<'a>>,
	submit_scene: F,
	selected: i32,
}

impl<'a, F> SceneSelector<'a, F>
where
	F: FnMut(Arc<MeshletSceneCpu>),
{
	pub async fn new(init: Arc<Init>, scenes: Vec<MeshletSceneFile<'a>>, submit_scene: F) -> io::Result<Self> {
		assert!(scenes.len() >= 1);
		let mut this = Self {
			init,
			scenes,
			submit_scene,
			selected: -1,
		};
		assert!(this.set_scene(0).await?);
		Ok(this)
	}

	pub async fn set_scene(&mut self, selected: i32) -> io::Result<bool> {
		let selected = i32::rem_euclid(selected, self.scenes.len() as i32);
		if selected == self.selected {
			return Ok(false);
		}
		self.selected = selected;
		let new_scene = self.scenes[selected as usize];
		println!("loading scene {:?}", new_scene);
		(self.submit_scene)(upload_scene(&self.init, new_scene).await?);
		Ok(true)
	}

	pub async fn handle_input(&mut self, event: &Event<()>) -> io::Result<()> {
		match event {
			Event::WindowEvent {
				event:
					WindowEvent::KeyboardInput {
						event:
							KeyEvent {
								state: Pressed,
								physical_key: Code { 0: code },
								..
							},
						..
					},
				..
			} => {
				use winit::keyboard::KeyCode::*;
				let mut selected = self.selected;
				match code {
					KeyT => selected -= 1,
					KeyG => selected += 1,
					_ => {}
				}
				self.set_scene(selected).await?;
			}
			_ => {}
		}
		Ok(())
	}
}

#[profiling::function]
async fn upload_scene(init: &Arc<Init>, scene_file: MeshletSceneFile<'_>) -> io::Result<Arc<MeshletSceneCpu>> {
	let scene = scene_file.load()?;
	let uploader = Uploader::new(
		init.bindless.clone(),
		init.memory_allocator.clone(),
		init.cmd_buffer_allocator.clone(),
		init.queues.client.transfer.clone(),
	);
	let cpu = scene.root().upload(&uploader).await.unwrap();
	Ok(Arc::new(cpu))
}
