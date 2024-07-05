use crate::meshlet::error::Error;
use crate::meshlet::process::Gltf;
use anyhow::Context;
use rayon::prelude::*;
use std::fs;
use std::fs::File;
use std::path::{Component, Path, PathBuf};

pub struct ProcessedModel {
	pub src_path: PathBuf,
	pub relative: Vec<String>,
	pub out_path: PathBuf,
}

#[profiling::function]
pub fn build(models_dir: &Path, out_dir: &Path, rerun_if_changed: bool) -> anyhow::Result<Vec<ProcessedModel>> {
	let models_dir = fs::canonicalize(models_dir).map_err(Error::from)?;
	if rerun_if_changed {
		println!("cargo:rerun-if-changed={}", models_dir.to_str().unwrap());
	}

	let model_paths = {
		profiling::scope!("search models");
		walkdir::WalkDir::new(&models_dir)
			.follow_links(true)
			.into_iter()
			.filter_map(|e| e.ok())
			.filter(|e| e.file_type().is_file())
			.filter(|e| e.path().extension().map_or(false, |ext| ext == "gltf" || ext == "glb"))
			.map(|e| {
				let src_path = e.into_path();
				// let relative = String::from(src_path.strip_prefix(&models_dir).unwrap().to_str().unwrap());
				// let relative = src_path.components().skip(models_dir.components().count()).filter(||).collect::<Vec<_>>();

				let relative = src_path
					.parent()
					.unwrap()
					.strip_prefix(&models_dir)
					.unwrap()
					.components()
					.filter_map(|c| match c {
						Component::Normal(s) => Some(s),
						_ => None,
					})
					.chain([src_path.file_stem().unwrap()])
					.map(|s| String::from(s.to_str().unwrap()))
					.collect::<Vec<_>>();
				let out_path = out_dir.join(format!("{}.bin.zstd", relative.join("/")));
				ProcessedModel {
					src_path,
					relative,
					out_path,
				}
			})
			.collect::<Vec<_>>()
	};

	{
		profiling::scope!("processing all models");
		model_paths
			.par_iter()
			.map(|model| {
				profiling::scope!("processing model", model.src_path.to_str().unwrap());
				let gltf = Gltf::open(model.src_path.clone())
					.with_context(|| format!("opening gltf file failed {:?}", model.src_path))?;
				let disk = gltf
					.process()
					.with_context(|| format!("processing gltf failed {:?}", model.src_path))?;
				fs::create_dir_all(model.out_path.parent().unwrap())
					.with_context(|| format!("failed creating output directories for file {:?}", model.out_path))?;
				let out_file = File::create(&model.out_path)
					.with_context(|| format!("failed creating output file {:?}", model.out_path))?;
				disk.serialize_compress_to(out_file)
					.with_context(|| format!("zstd stream failed writing {:?}", model.out_path))?;
				Ok::<(), anyhow::Error>(())
			})
			.collect::<Result<_, _>>()?;
	}

	Ok(model_paths)
}
