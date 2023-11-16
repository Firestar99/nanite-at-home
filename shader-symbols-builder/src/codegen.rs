use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::{fs, io};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use smallvec::SmallVec;

pub struct CodegenOptions {
	pub shader_symbols_path: String,
}

#[derive(Debug)]
pub enum CodegenError {
	IOError(io::Error),
	#[cfg(feature = "use-pretty-print")]
	SynError(syn::Error),
}

impl Display for CodegenError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			CodegenError::IOError(e) => {
				write!(f, "IO error: {}", e)
			}
			#[cfg(feature = "use-pretty-print")]
			CodegenError::SynError(e) => {
				write!(f, "Syn parsing failed: {}", e)
			}
		}
	}
}

impl Error for CodegenError {}

pub fn codegen_shader_symbols<'a>(
	shaders: impl Iterator<Item = (&'a str, &'a PathBuf)>,
	out_path: &PathBuf,
	_options: &CodegenOptions,
) -> Result<(), CodegenError> {
	let tokens = ModNode::new(shaders).emit();

	// when pretty printing fails, always write plain version, then error
	let (content, error) = codegen_try_pretty_print(tokens);
	let content = format!("{}{}", SHADER_TYPE_WARNING, content);
	fs::write(out_path, content).map_err(CodegenError::IOError)?;
	match error {
		None => Ok(()),
		Some(e) => Err(e),
	}
}

#[cfg(not(feature = "use-pretty-print"))]
pub fn codegen_try_pretty_print(tokens: TokenStream) -> (String, Option<CodegenError>) {
	(tokens.to_string(), None)
}

#[cfg(feature = "use-pretty-print")]
pub fn codegen_try_pretty_print(tokens: TokenStream) -> (String, Option<CodegenError>) {
	match syn::parse2(tokens.clone()) {
		Ok(parse) => (prettyplease::unparse(&parse), None),
		Err(e) => (tokens.to_string(), Some(CodegenError::SynError(e))),
	}
}

const SHADER_TYPE_WARNING: &str = stringify! {
	/// machine generated file, DO NOT EDIT

	/// Shaders may be of different types but are all declared as `ty: "vertex"` in the `shader!` macro.
	/// This is due to the macro validating that you specify a shader type, but when you supply your
	/// shader as `bytes: /path/to/*.spv` it won't actually use the shader type specified anywhere.
	/// It is only used when compiling from glsl source, which we are not doing.
};

#[derive(Debug, Default)]
struct ModNode<'a> {
	shader: Option<(&'a str, &'a PathBuf)>,
	children: HashMap<&'a str, ModNode<'a>>,
}

impl<'a> ModNode<'a> {
	fn new(shaders: impl Iterator<Item = (&'a str, &'a PathBuf)>) -> Self {
		let mut root = Self::default();
		for shader in shaders {
			root.insert(shader.0.split("::"), shader);
		}
		root
	}

	fn insert(&mut self, mut path: impl Iterator<Item = &'a str>, shader: (&'a str, &'a PathBuf)) {
		match path.next() {
			None => {
				assert!(matches!(self.shader, None), "Duplicate shader name!");
				self.shader = Some(shader);
			}
			Some(name) => {
				self.children.entry(name).or_default().insert(path, shader);
			}
		}
	}

	fn emit(&self) -> TokenStream {
		let content = self.emit_loop();
		quote! {
			#[allow(unused_imports)]
			use std::sync::Arc;
			#[allow(unused_imports)]
			use vulkano::device::Device;
			#[allow(unused_imports)]
			use vulkano::shader::EntryPoint;
			#[allow(unused_imports)]
			use vulkano_shaders::shader;

			#content
		}
	}

	fn emit_loop(&self) -> TokenStream {
		let mut content = quote! {};
		if let Some((full_name, path)) = self.shader {
			let path = path.to_str().unwrap();
			content = quote! {
				#content

				shader! {
					ty: "vertex",
					bytes: #path,
				}

				pub fn new(device: Arc<Device>) -> EntryPoint {
					load(device).unwrap().entry_point(#full_name).unwrap()
				}
			}
		}

		if !self.children.is_empty() {
			let mut children = self.children.iter().collect::<SmallVec<[_; 5]>>();
			children.sort_unstable_by(|(k1, _), (k2, _)| k1.cmp(k2));

			for (name, node) in children {
				let name = format_ident!("{}", name);
				let inner = node.emit_loop();
				content = quote! {
					#content

					pub mod #name {
						use super::*;
						#inner
					}
				}
			}
		}
		content
	}
}
