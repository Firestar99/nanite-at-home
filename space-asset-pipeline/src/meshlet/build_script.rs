use crate::gltf::{find_gltf_files, Gltf};
use crate::gltf::{to_mod_hierarchy, GltfFile};
use crate::meshlet::process::process_meshlets;
use anyhow::Context;
use rayon::prelude::*;
use std::fs;
use std::fs::File;
use std::path::Path;

#[profiling::function]
pub fn build_script(
	models_dir: &Path,
	out_dir: &Path,
	models_rs: &Path,
	rerun_if_changed: bool,
) -> anyhow::Result<Vec<GltfFile>> {
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
			.collect::<Result<_, _>>()?;
	}

	{
		profiling::scope!("writing models mod hierarchy");
		fs::write(models_rs, to_mod_hierarchy(model_paths.iter())?.to_string())?;
	}

	Ok(model_paths)
}
