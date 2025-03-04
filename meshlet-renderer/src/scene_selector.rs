use egui::Ui;
use rust_gpu_bindless::descriptor::Bindless;
use space_asset_disk::meshlet::scene::MeshletSceneFile;
use space_asset_rt::meshlet::scene::{upload_scene, MeshletSceneCpu};
use space_asset_rt::uploader::Uploader;
use std::io;
use std::sync::Arc;

pub struct SceneSelector<'a> {
	bindless: Arc<Bindless>,
	scenes: Vec<MeshletSceneFile<'a>>,
	loaded_scene: Option<Arc<MeshletSceneCpu>>,
	selected: i32,
	prev_selected: i32,
}

impl<'a> SceneSelector<'a> {
	pub fn new(bindless: Arc<Bindless>, scenes: Vec<MeshletSceneFile<'a>>) -> Self {
		Self {
			bindless,
			scenes,
			loaded_scene: None,
			selected: 0,
			prev_selected: -1,
		}
	}

	pub fn set_scene(&mut self, selected: i32) {
		self.selected = i32::rem_euclid(selected, self.scenes.len() as i32);
	}

	pub async fn get_or_load_scene(&mut self) -> io::Result<&Arc<MeshletSceneCpu>> {
		if self.prev_selected != self.selected {
			self.prev_selected = self.selected;

			let new_scene = self.scenes[self.selected as usize];
			println!("loading scene {:?}", new_scene);
			let scene = load_scene(&self.bindless, new_scene).await?;
			{
				println!("{} instances", scene.num_instances);
			}
			self.loaded_scene = Some(scene);
		}
		Ok(self.loaded_scene.as_ref().unwrap())
	}

	pub fn ui(&mut self, ui: &mut Ui) {
		let mut newsel = self.selected;
		ui.strong("Scene:");
		egui::ComboBox::from_id_salt(concat!(file!(), line!()))
			.selected_text(format!("{}", self.scenes[newsel as usize].name()))
			.show_ui(ui, |ui| {
				for i in 0..self.scenes.len() {
					ui.selectable_value(&mut newsel, i as i32, format!("{}", self.scenes[i].name()));
				}
			});
		self.set_scene(newsel);
	}
}

#[profiling::function]
async fn load_scene(bindless: &Arc<Bindless>, scene_file: MeshletSceneFile<'_>) -> io::Result<Arc<MeshletSceneCpu>> {
	let scene = scene_file.load()?;
	let uploader = Uploader::new(bindless.clone());
	let cpu = upload_scene(scene.root(), &uploader).await.unwrap();
	Ok(Arc::new(cpu))
}
