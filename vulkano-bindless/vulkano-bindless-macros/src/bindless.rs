use crate::image_types::standard_image_types;
use crate::symbols::Symbols;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Error, FnArg, ItemFn, PatType, Result, ReturnType, Token};

pub fn bindless(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> Result<TokenStream> {
	let symbols = Symbols::new();

	let item = syn::parse::<ItemFn>(item)?;

	match item.sig.output {
		ReturnType::Default => (),
		ReturnType::Type(_, e) => return Err(Error::new(e.span(), "Entry points must not return anything!")),
	}

	let args_parse = item
		.sig
		.inputs
		.iter()
		.map(|arg| {
			let arg = match arg {
				FnArg::Receiver(e) => {
					return Err(Error::new(
						e.span(),
						"Entry points may not contain a receiver argument!",
					))
				}
				FnArg::Typed(e) => e,
			};
			let mut iter_bindless = arg.attrs.iter().filter(|attr| attr.path().is_ident(&symbols.bindless));
			if let Some(bindless) = iter_bindless.next() {
				if iter_bindless.next().is_some() {
					return Err(Error::new(
						arg.span(),
						"Argument must have at most one bindless attribute!",
					));
				}
				bindless_parse_args(&symbols, arg, bindless)
			} else {
				let var_name = &arg.pat;
				Ok((quote!(#arg,), quote!(#var_name,)))
			}
		})
		.collect::<Result<Vec<_>>>()?;

	let (fn_args_outer, fn_values_inner): (TokenStream, TokenStream) = args_parse.into_iter().unzip();

	let fn_args_inner: Punctuated<PatType, Token![,]> = item
		.sig
		.inputs
		.iter()
		.map(|arg| {
			let arg = match arg {
				FnArg::Receiver(_) => unreachable!(),
				FnArg::Typed(e) => e,
			};
			PatType {
				attrs: Vec::new(),
				..arg.clone()
			}
		})
		.collect();

	let entry_point_attr = TokenStream::from(attr);
	let vis = &item.vis;
	let fn_ident_outer = &item.sig.ident;
	let fn_ident_inner = format_ident!("__bindless_{}", fn_ident_outer);
	let fn_block_inner = &item.block;
	let crate_ident = symbols.crate_ident;

	// the fn_ident_inner *could* be put within the entry point fn,
	// but putting it outside significantly improves editor performance in rustrover
	Ok(quote! {
		#[#crate_ident::spirv(#entry_point_attr)]
		#[allow(clippy::too_many_arguments)]
		#vis fn #fn_ident_outer(#fn_args_outer) {
			#fn_ident_inner(#fn_values_inner);
		}

		#[allow(clippy::too_many_arguments)]
		fn #fn_ident_inner(#fn_args_inner) #fn_block_inner
	})
}

#[allow(clippy::single_match)]
fn bindless_parse_args(symbols: &Symbols, arg: &PatType, bindless: &Attribute) -> Result<(TokenStream, TokenStream)> {
	let bindless_args = bindless.meta.require_list()?;
	match bindless_args.tokens.to_string().as_str() {
		"descriptors" => bindless_descriptors(symbols),
		_ => Err(Error::new(arg.span(), "Unknown bindless parameter")),
	}
}

fn bindless_descriptors(symbols: &Symbols) -> Result<(TokenStream, TokenStream)> {
	let crate_ident = &symbols.crate_ident;
	let image_args;
	let image_values;
	macro_rules! make_image_args {
		(
			{$($storage_name:ident: $storage_ty:ty,)*}
			{$($sampled_name:ident: $sampled_ty:ty,)*}
		) => {
			$(let $storage_name = format_ident!("__bindless_{}", stringify!($storage_name));)*
			$(let $sampled_name = format_ident!("__bindless_{}", stringify!($sampled_name));)*

			image_args = quote! {
				$(#[spirv(descriptor_set = 0, binding = 1)] #$storage_name: &#crate_ident::spirv_std::RuntimeArray<#crate_ident$storage_ty>,)*
				$(#[spirv(descriptor_set = 0, binding = 2)] #$sampled_name: &#crate_ident::spirv_std::RuntimeArray<#crate_ident$sampled_ty>,)*
			};
			image_values = quote! {
				$($storage_name: #$storage_name,)*
				$($sampled_name: #$sampled_name,)*
			};
		};
	}
	standard_image_types!(make_image_args);

	let buffers = format_ident!("__bindless_buffers");
	let samplers = format_ident!("__bindless_samplers");

	// these "plain" spirv here are correct, as they are non-macro attributes to function arguments, not proc macros!
	let args = quote! {
		#[spirv(descriptor_set = 0, binding = 0, storage_buffer)] #buffers: &mut #crate_ident::spirv_std::RuntimeArray<[u32]>,
		#image_args
		#[spirv(descriptor_set = 0, binding = 3)] #samplers: &#crate_ident::spirv_std::RuntimeArray<#crate_ident::descriptor::Sampler>,
	};
	let values = quote! {
		&#crate_ident::descriptor::Descriptors {
			buffers: #buffers,
			#image_values
			samplers: #samplers,
		},
	};
	Ok((args, values))
}
