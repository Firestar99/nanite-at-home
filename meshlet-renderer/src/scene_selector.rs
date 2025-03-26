use egui::{SliderClamping, Ui, Widget};
use glam::UVec3;
use rust_gpu_bindless::descriptor::Bindless;
use space_asset_disk::meshlet::scene::MeshletSceneFile;
use space_asset_rt::meshlet::scene::{upload_scene, InstancedMeshletSceneCpu, MeshletSceneCpu};
use space_asset_rt::uploader::Uploader;
use std::sync::Arc;

pub struct SceneSelector<'a> {
	bindless: Bindless,
	scenes: Vec<MeshletSceneFile<'a>>,
	loaded_scene: Option<Arc<MeshletSceneCpu>>,
	loaded_scene_instance: Option<InstancedMeshletSceneCpu>,
	selected: i32,
	prev_selected: i32,
	instance_count: UVec3,
}

impl<'a> SceneSelector<'a> {
	pub fn new(bindless: Bindless, scenes: Vec<MeshletSceneFile<'a>>) -> Self {
		Self {
			bindless,
			scenes,
			loaded_scene: None,
			loaded_scene_instance: None,
			selected: 0,
			prev_selected: -1,
			instance_count: UVec3::ONE,
		}
	}

	pub fn set_scene(&mut self, selected: i32) {
		self.selected = i32::rem_euclid(selected, self.scenes.len() as i32);
	}

	pub async fn get_or_load_scene(&mut self) -> anyhow::Result<&InstancedMeshletSceneCpu> {
		let mut rebuild_instance = false;
		if self.prev_selected != self.selected {
			self.prev_selected = self.selected;

			let new_scene = self.scenes[self.selected as usize];
			println!("loading scene {:?}", new_scene);
			let scene = load_scene(&self.bindless, new_scene).await?;
			self.loaded_scene = Some(scene);
			self.instance_count = UVec3::ONE;
			rebuild_instance = true;
		}
		let scene = self.loaded_scene.as_ref().unwrap();

		if self
			.loaded_scene_instance
			.as_ref()
			.map_or(true, |i| i.instance_count != self.instance_count)
		{
			rebuild_instance = true;
		}
		if rebuild_instance {
			self.loaded_scene_instance = Some(scene.instantiate(&self.bindless, self.instance_count)?);
		}

		Ok(self.loaded_scene_instance.as_ref().unwrap())
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

		ui.collapsing("Scene stats", |ui| {
			if let Some(scene) = self.loaded_scene.as_ref() {
				egui::Grid::new("Scene stats grid").show(ui, |ui| {
					ui.label("vertices source");
					ui.label(format!("{}", scene.stats.source.unique_vertices));
					ui.end_row();

					ui.label("triangles");
					ui.label(format!("{}", scene.stats.source.triangles));
					ui.end_row();

					ui.label("meshlets");
					ui.label(format!("{}", scene.stats.source.meshlets));
					ui.end_row();

					ui.label("meshlet vertices");
					ui.label(format!("{}", scene.stats.source.meshlet_vertices));
					ui.end_row();

					ui.label("bounds min");
					ui.label(format!("{:?}", scene.stats.source.bounds_min));
					ui.end_row();

					ui.label("bounds max");
					ui.label(format!("{:?}", scene.stats.source.bounds_max));
					ui.end_row();
				});
			} else {
				ui.label("No scene loaded");
			}
		});

		egui::Slider::new(&mut self.instance_count.x, 1..=10)
			.clamping(SliderClamping::Never)
			.text("Instances X")
			.ui(ui);

		egui::Slider::new(&mut self.instance_count.y, 1..=10)
			.clamping(SliderClamping::Never)
			.text("Instances Y")
			.ui(ui);
	}
}

async fn load_scene(bindless: &Bindless, scene_file: MeshletSceneFile<'_>) -> anyhow::Result<Arc<MeshletSceneCpu>> {
	profiling::function_scope!();
	let scene = scene_file.load()?;
	let uploader = Uploader::new(bindless.clone());
	let cpu = upload_scene(scene.root(), &uploader).await?;
	Ok(Arc::new(cpu))
}
