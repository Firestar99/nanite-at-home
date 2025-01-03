use rust_gpu_bindless::descriptor::Bindless;
use space_asset_disk::meshlet::scene::MeshletSceneFile;
use space_asset_rt::meshlet::scene::{upload_scene, MeshletSceneCpu};
use space_asset_rt::uploader::Uploader;
use std::io;
use std::sync::Arc;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::keyboard::PhysicalKey::Code;

pub struct SceneSelector<'a, F>
where
	F: FnMut(Arc<MeshletSceneCpu>),
{
	bindless: Arc<Bindless>,
	scenes: Vec<MeshletSceneFile<'a>>,
	submit_scene: F,
	selected: i32,
}

impl<'a, F> SceneSelector<'a, F>
where
	F: FnMut(Arc<MeshletSceneCpu>),
{
	pub async fn new(bindless: Arc<Bindless>, scenes: Vec<MeshletSceneFile<'a>>, submit_scene: F) -> io::Result<Self> {
		assert!(!scenes.is_empty());
		let mut this = Self {
			bindless,
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
		let scene = load_scene(&self.bindless, new_scene).await?;
		{
			let num_instances = scene.num_instances;
			println!("{} instances", num_instances);
		}
		(self.submit_scene)(scene);
		Ok(true)
	}

	pub async fn handle_input(&mut self, event: &Event<()>) -> io::Result<()> {
		if let Event::WindowEvent {
			event:
				WindowEvent::KeyboardInput {
					event:
						KeyEvent {
							state: ElementState::Pressed,
							physical_key: Code { 0: code },
							..
						},
					..
				},
			..
		} = event
		{
			use winit::keyboard::KeyCode::*;
			let mut selected = self.selected;
			match code {
				KeyT => selected -= 1,
				KeyG => selected += 1,
				_ => {}
			}
			self.set_scene(selected).await?;
		}
		Ok(())
	}
}

#[profiling::function]
async fn load_scene(bindless: &Arc<Bindless>, scene_file: MeshletSceneFile<'_>) -> io::Result<Arc<MeshletSceneCpu>> {
	let scene = scene_file.load()?;
	let uploader = Uploader::new(bindless.clone());
	let cpu = upload_scene(scene.root(), &uploader).await.unwrap();
	Ok(Arc::new(cpu))
}
