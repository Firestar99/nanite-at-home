use crate::symbols::find_rust_gpu_bindless;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rust_gpu_bindless_macro_utils::modnode::ModNode;
use std::borrow::Cow;
use std::ffi::CString;
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use syn::Path;

pub struct CodegenOptions {
	pub shader_symbols_path: String,
}

pub fn codegen_shader_symbols<'a>(
	shaders: impl Iterator<Item = (&'a str, &'a PathBuf)>,
	crate_name: &String,
	out_path: &PathBuf,
	_options: &CodegenOptions,
) -> anyhow::Result<()> {
	let crate_name = format_ident!("{}", crate_name);
	let rust_gpu_bindless = find_rust_gpu_bindless()?;

	let mut root = ModNode::root();
	for shader in shaders {
		root.insert(shader.0.split("::").map(Cow::Borrowed), shader)?;
	}
	let tokens = root.to_tokens(|shader_ident, (entry_point_name, spv_path)| {
		let mut mod_path = syn::parse_str::<Path>(entry_point_name).unwrap();
		mod_path.segments.pop();
		let entry_point_name = CString::new(*entry_point_name).unwrap();

		let mut spv_file = File::open(PathBuf::from(spv_path)).unwrap();
		let spv_binary = ash::util::read_spv(&mut spv_file).unwrap();

		// same formatting in macros and shader-builder
		let entry_shader_type_ident = format_ident!("__Bindless_{}_ShaderType", shader_ident);
		let param_type_ident = format_ident!("__Bindless_{}_ParamConstant", shader_ident);

		// FIXME: dynamically select core or bindless!!!
		quote! {
			pub struct #shader_ident;

			impl #rust_gpu_bindless::__private::shader::BindlessShader for #shader_ident {
				type ShaderType = #crate_name::#mod_path #entry_shader_type_ident;
				type ParamConstant = #crate_name::#mod_path #param_type_ident;

				fn spirv_binary(&self) -> &#rust_gpu_bindless::__private::shader::SpirvBinary<'static> {
					&#rust_gpu_bindless::__private::shader::SpirvBinary {
						binary: &[#(#spv_binary),*],
						entry_point_name: #entry_point_name,
					}
				}
			}

			impl #shader_ident {
				pub fn new() -> &'static #shader_ident {
					&#shader_ident {}
				}
			}
		}
	});

	// when pretty printing fails, always write plain version, then error
	let (content, error) = codegen_try_pretty_print(tokens);
	fs::write(out_path, content)?;
	eprintln!("Shader file written to {}", out_path.display());
	if let Some(e) = error { Err(e)? } else { Ok(()) }
}

#[cfg(not(feature = "use-pretty-print"))]
pub fn codegen_try_pretty_print(tokens: TokenStream) -> (String, Option<syn::Error>) {
	(tokens.to_string(), None)
}

#[cfg(feature = "use-pretty-print")]
pub fn codegen_try_pretty_print(tokens: TokenStream) -> (String, Option<syn::Error>) {
	match syn::parse2(tokens.clone()) {
		Ok(parse) => (prettyplease::unparse(&parse), None),
		Err(e) => (tokens.to_string(), Some(e)),
	}
}
