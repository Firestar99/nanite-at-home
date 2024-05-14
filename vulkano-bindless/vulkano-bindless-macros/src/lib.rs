use proc_macro::TokenStream;
use quote::ToTokens;
use syn::Error;

mod bindless;
mod symbols;

#[path = "../../image_types.rs"]
mod image_types;

#[proc_macro_attribute]
pub fn bindless(attr: TokenStream, item: TokenStream) -> TokenStream {
	bindless::bindless(attr, item)
		.unwrap_or_else(Error::into_compile_error)
		.into()
}

trait AppendTokens {
	fn append_tokens(&mut self, tokens: impl ToTokens);
}

impl AppendTokens for proc_macro2::TokenStream {
	fn append_tokens(&mut self, tokens: impl ToTokens) {
		tokens.to_tokens(self)
	}
}
