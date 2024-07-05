use crate::meshlet::build_script::ProcessedModel;
use crate::modnode::{ModNode, ModNodeError, ModNodeToTokens};
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use std::path::Path;

#[profiling::function]
pub fn codegen<'a>(model_paths: impl Iterator<Item = &'a ProcessedModel>) -> Result<TokenStream, ModNodeError> {
	let mut root = ModNode::root();
	for model in model_paths {
		root.insert(
			model
				.relative
				.iter()
				.map(|s| s.chars().map(filter_chars_for_typename).collect::<String>().into()),
			OutputTokens {
				out_path: &model.out_path,
			},
		)?;
	}
	Ok(root.to_tokens())
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

struct OutputTokens<'a> {
	out_path: &'a Path,
}

impl<'a> ModNodeToTokens for OutputTokens<'a> {
	fn to_tokens_with_ident(&self, name: Ident) -> TokenStream {
		let out_path = self.out_path.to_str().unwrap();
		quote! {
			pub const #name: &[u8] = include_bytes!(#out_path);
		}
	}
}
