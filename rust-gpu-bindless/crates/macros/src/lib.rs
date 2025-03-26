use crate::buffer_struct::BufferStructType;
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::Error;

mod assert_transfer_size;
mod bindless;
mod buffer_struct;
mod symbols;

#[path = "../../image_types.rs"]
mod image_types;

#[proc_macro_attribute]
pub fn bindless(attr: TokenStream, item: TokenStream) -> TokenStream {
	bindless::bindless(attr, item)
		.unwrap_or_else(Error::into_compile_error)
		.into()
}

#[proc_macro_derive(BufferStruct)]
pub fn buffer_struct(content: TokenStream) -> TokenStream {
	buffer_struct::buffer_struct(BufferStructType::Default, content)
		.unwrap_or_else(Error::into_compile_error)
		.into()
}

#[proc_macro_derive(BufferStructPlain)]
pub fn buffer_struct_plain(content: TokenStream) -> TokenStream {
	buffer_struct::buffer_struct(BufferStructType::Plain, content)
		.unwrap_or_else(Error::into_compile_error)
		.into()
}

#[proc_macro]
pub fn assert_transfer_size(content: TokenStream) -> TokenStream {
	assert_transfer_size::assert_transfer_size(content)
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
