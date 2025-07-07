use proc_macro2::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote, TokenStreamExt};
use rust_gpu_bindless_macro_utils::modnode::{ModNode, ModNodeError};
use smallvec::SmallVec;
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug)]
pub struct GltfFile {
	pub src_path: PathBuf,
	pub relative: PathBuf,
	pub out_relative: PathBuf,
	pub out_path: PathBuf,
}

pub fn find_gltf_files(models_dir: &Path, out_dir: &Path, print_rerun_if_changed: bool) -> io::Result<Vec<GltfFile>> {
	profiling::function_scope!();
	let models_dir = fs::canonicalize(models_dir)?;
	if print_rerun_if_changed {
		println!("cargo:rerun-if-changed={}", models_dir.to_str().unwrap());
	}

	Ok(walkdir::WalkDir::new(&models_dir)
		.follow_links(true)
		.into_iter()
		.filter_map(|e| e.ok())
		.filter(|e| e.file_type().is_file())
		.filter(|e| e.path().extension().is_some_and(|ext| ext == "gltf" || ext == "glb"))
		.map(|e| {
			let src_path = e.into_path();
			let relative = src_path.strip_prefix(&models_dir).unwrap().to_path_buf();
			let out_relative = relative.with_file_name(format!(
				"{}.bin",
				relative.file_name().map(|c| c.to_string_lossy()).unwrap_or_default()
			));
			let out_path = out_dir.join(&out_relative);
			GltfFile {
				src_path,
				relative,
				out_relative,
				out_path,
			}
		})
		.collect::<Vec<_>>())
}

pub fn to_mod_hierarchy<'a>(model_paths: impl Iterator<Item = &'a GltfFile>) -> Result<TokenStream, ModNodeError> {
	profiling::function_scope!();
	let found_crate = crate_name("space-asset-disk").unwrap();
	let crate_name = match &found_crate {
		FoundCrate::Itself => "crate",
		FoundCrate::Name(name) => name,
	};
	let crate_name = format_ident!("{}", crate_name);

	let mut root = ModNode::root();
	for model in model_paths {
		root.insert(
			model
				.relative
				.components()
				.filter_map(|c| c.as_os_str().to_str())
				.map(|s| s.chars().map(filter_chars_for_typename).collect::<String>().into()),
			model,
		)?;
	}
	let all_models = {
		let mut model_paths = Vec::new();
		root.iter(|path, _| {
			model_paths.push(SmallVec::<[&str; 6]>::from(path));
		});
		model_paths.sort();
		let mut all_models = quote!();
		for path in model_paths {
			all_models.append_separated(path.iter().map(|name| format_ident!("{}", name)), quote!(::));
			all_models.append_all(&[quote!(,)])
		}
		quote! {
			pub const ALL_MODELS: &[#crate_name::meshlet::scene::MeshletSceneFile<'static>] = &[
				#all_models
			];
		}
	};
	let mod_hierarchy = root.to_tokens(|name, model| {
		let relative = &model.relative.to_string_lossy();
		let out_relative = &model.out_relative.to_string_lossy();
		quote! {
			pub const #name: #crate_name::meshlet::scene::MeshletSceneFile<'static> = unsafe { #crate_name::meshlet::scene::MeshletSceneFile::new(#relative, #out_relative) };
		}
	});
	Ok(quote! {
		#all_models
		#mod_hierarchy
	})
}

fn filter_chars_for_typename(c: char) -> char {
	if c.is_ascii_lowercase() || c.is_ascii_uppercase() || c.is_ascii_digit() {
		c
	} else {
		'_'
	}
}
