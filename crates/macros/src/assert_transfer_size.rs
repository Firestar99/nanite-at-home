use crate::symbols::Symbols;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Token, Type};

#[allow(dead_code)]
struct AssertTransferSizeInput {
	ty: Type,
	comma: Token![,],
	size: Expr,
}

impl Parse for AssertTransferSizeInput {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			ty: input.parse()?,
			comma: input.parse()?,
			size: input.parse()?,
		})
	}
}

pub fn assert_transfer_size(content: proc_macro::TokenStream) -> syn::Result<TokenStream> {
	let input = syn::parse::<AssertTransferSizeInput>(content)?;
	let symbols = Symbols::new()?;
	let crate_buffer_content = &symbols.crate_buffer_content;
	let ty = input.ty;
	let size = input.size;
	let trait_ident = match symbols.crate_shaders() {
		Ok(_) => format_ident!("BufferStruct"),
		Err(_) => format_ident!("BufferStructPlain"),
	};
	Ok(quote! {
		#crate_buffer_content::__private::static_assertions::const_assert_eq!(::core::mem::size_of::<<#ty as #crate_buffer_content::#trait_ident>::Transfer>(), #size);
	})
}
