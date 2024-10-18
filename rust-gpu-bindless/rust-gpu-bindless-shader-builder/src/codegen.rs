use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::{fs, io, ptr};
use syn::punctuated::Punctuated;
use syn::{Path, PathSegment, Token};

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
	crate_name: &String,
	out_path: &PathBuf,
	_options: &CodegenOptions,
) -> Result<(), CodegenError> {
	let tokens = ModNode::new(shaders).emit(crate_name);

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
	// machine generated file, DO NOT EDIT
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
				assert!(self.shader.is_none(), "Duplicate shader name!");
				self.shader = Some(shader);
			}
			Some(name) => {
				self.children.entry(name).or_default().insert(path, shader);
			}
		}
	}

	fn emit(&self, crate_name: &String) -> TokenStream {
		let crate_name = format_ident!("{}", crate_name);
		self.emit_loop(&crate_name)
	}

	fn emit_loop(&self, crate_name: &Ident) -> TokenStream {
		let mut content: SmallVec<[_; 5]> = SmallVec::new();
		if let Some((full_name, path)) = self.shader {
			let path = path.to_str().unwrap();
			let shader_ident = format_ident!("BindlessShaderImpl");

			println!("{}", full_name);
			let full_name = syn::parse_str::<Path>(full_name).unwrap();
			let entry_ident = &full_name.segments.iter().last().unwrap().ident;
			let mod_path = full_name
				.segments
				.iter()
				.take_while(|i| !ptr::eq(&i.ident, entry_ident))
				.collect::<Punctuated<&PathSegment, Token![::]>>();

			// same formatting in macros and shader-builder
			let entry_shader_type_ident = format_ident!("__Bindless_{}_ShaderType", entry_ident);
			let param_type_ident = format_ident!("__Bindless_{}_ParamConstant", entry_ident);

			content.push(quote! {
				vulkano_shaders::shader! {
					bytes: #path,
					generate_structs: false,
				}

				pub struct #shader_ident;

				impl rust_gpu_bindless::pipeline::shader::BindlessShader for #shader_ident {
					type ShaderType = #crate_name::#mod_path::#entry_shader_type_ident;
					type ParamConstant = #crate_name::#mod_path::#param_type_ident;

					fn load(&self, device: std::sync::Arc<vulkano::device::Device>) -> Result<std::sync::Arc<vulkano::shader::ShaderModule>, vulkano::Validated<vulkano::VulkanError>> {
						load(device)
					}
				}

				pub fn new() -> &'static #shader_ident {
					&#shader_ident {}
				}
			});
		}

		if !self.children.is_empty() {
			let mut children = self.children.iter().collect::<SmallVec<[_; 5]>>();
			children.sort_unstable_by(|(k1, _), (k2, _)| k1.cmp(k2));

			for (name, node) in children {
				let name = format_ident!("{}", name);
				let inner = node.emit_loop(crate_name);
				content.push(quote! {
					pub mod #name {
						#inner
					}
				})
			}
		}
		content.into_iter().collect::<TokenStream>()
	}
}
