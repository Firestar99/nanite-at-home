use crate::gltf::{find_gltf_files, Gltf};
use crate::gltf::{to_mod_hierarchy, GltfFile};
use crate::meshlet::process::process_meshlets;
use anyhow::Context;
use rayon::prelude::*;
use space_asset_disk::meshlet::scene::EXPORT_FOLDER_NAME;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::{env, fs};

pub fn out_and_export_dir() -> Option<(PathBuf, PathBuf)> {
	let out_dir = PathBuf::from(&env::var("OUT_DIR").unwrap());
	let mut export_dir = PathBuf::from(out_dir.parent()?.parent()?.parent()?);
	export_dir.push(EXPORT_FOLDER_NAME);
	Some((out_dir, export_dir))
}

pub fn build_script(
	models_dir: &Path,
	out_dir: &Path,
	models_rs: Option<&Path>,
	rerun_if_changed: bool,
) -> anyhow::Result<Vec<GltfFile>> {
	profiling::function_scope!();
	let model_paths = find_gltf_files(models_dir, out_dir, rerun_if_changed)?;

	{
		profiling::scope!("processing all models");
		model_paths
			.par_iter()
			.map(|model| {
				profiling::scope!("processing model", model.src_path.to_str().unwrap());
				let gltf = Gltf::open(&model.src_path)
					.with_context(|| format!("opening gltf file failed {:?}", model.src_path))?;
				let disk =
					process_meshlets(&gltf).with_context(|| format!("processing gltf failed {:?}", model.src_path))?;
				fs::create_dir_all(model.out_path.parent().unwrap())
					.with_context(|| format!("failed creating output directories for file {:?}", model.out_path))?;
				let out_file = File::create(&model.out_path)
					.with_context(|| format!("failed creating output file {:?}", model.out_path))?;
				disk.serialize_to(out_file)
					.with_context(|| format!("zstd stream failed writing {:?}", model.out_path))?;
				Ok::<(), anyhow::Error>(())
			})
			.collect::<Result<(), _>>()?;
	}

	if let Some(models_rs) = models_rs {
		profiling::scope!("writing models mod hierarchy");
		fs::write(models_rs, to_mod_hierarchy(model_paths.iter())?.to_string())?;
	}

	Ok(model_paths)
}
