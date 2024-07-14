use crate::modnode::{ModNode, ModNodeError};
use proc_macro2::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use std::path::{Component, Path, PathBuf};
use std::{fs, io};

pub struct GltfFile {
	pub src_path: PathBuf,
	pub relative: Vec<String>,
	pub out_relative: String,
	pub out_path: PathBuf,
}

#[profiling::function]
pub fn find_gltf_files(models_dir: &Path, out_dir: &Path, print_rerun_if_changed: bool) -> io::Result<Vec<GltfFile>> {
	let models_dir = fs::canonicalize(models_dir)?;
	if print_rerun_if_changed {
		println!("cargo:rerun-if-changed={}", models_dir.to_str().unwrap());
	}

	Ok(walkdir::WalkDir::new(&models_dir)
		.follow_links(true)
		.into_iter()
		.filter_map(|e| e.ok())
		.filter(|e| e.file_type().is_file())
		.filter(|e| e.path().extension().map_or(false, |ext| ext == "gltf" || ext == "glb"))
		.map(|e| {
			let src_path = e.into_path();
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
			let out_relative = format!("{}.bin", relative.join("/"));
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

#[profiling::function]
pub fn to_mod_hierarchy<'a>(model_paths: impl Iterator<Item = &'a GltfFile>) -> Result<TokenStream, ModNodeError> {
	let found_crate = crate_name("space-asset").unwrap();
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
				.iter()
				.map(|s| s.chars().map(filter_chars_for_typename).collect::<String>().into()),
			model,
		)?;
	}
	Ok(root.to_tokens(|name, model| {
		let out_relative = &model.out_relative;
		quote! {
			pub const #name: #crate_name::meshlet::scene::MeshletSceneFile<'static> = unsafe { #crate_name::meshlet::scene::MeshletSceneFile::new(#out_relative) };
		}
	}))
}

fn filter_chars_for_typename(c: char) -> char {
	if 'a' <= c && c <= 'z' {
		c
	} else if 'A' <= c && c <= 'Z' {
		c
	} else if '0' <= c && c <= '9' {
		c
	} else if c == '_' {
		c
	} else {
		'_'
	}
}
