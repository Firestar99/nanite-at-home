use crate::image_types::standard_image_types;
use crate::symbols::Symbols;
use crate::AppendTokens;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::spanned::Spanned;
use syn::{Error, FnArg, ItemFn, PatType, Result, ReturnType, Type, TypeReference};

pub struct BindlessContext<'a> {
	symbols: &'a Symbols,
	item: &'a ItemFn,
	entry_args: TokenStream,
	inner_value: TokenStream,
	inner_arg: TokenStream,
}

pub fn bindless(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> Result<TokenStream> {
	let symbols = Symbols::new();
	let item = syn::parse::<ItemFn>(item)?;
	match &item.sig.output {
		ReturnType::Default => (),
		ReturnType::Type(_, e) => return Err(Error::new(e.span(), "Entry points must not return anything!")),
	}

	let mut bindless_param = None;
	let mut bindless_descriptors = None;
	let mut forward = Vec::new();
	for arg in item.sig.inputs.iter() {
		let arg = match arg {
			FnArg::Receiver(e) => {
				return Err(Error::new(
					e.span(),
					"Entry points may not contain a receiver (eg. self) argument!",
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
			let bindless_list = bindless.meta.require_list()?;
			let bindless_list_str = bindless_list.tokens.to_string();
			let slot = match &*bindless_list_str {
				"param_constants" => &mut bindless_param,
				"descriptors" => &mut bindless_descriptors,
				_ => return Err(Error::new(arg.span(), "Unknown bindless parameter")),
			};
			if let Some(old) = slot.replace(arg) {
				let mut error = Error::new(
					old.span(),
					format!("Function must only have one argument with #[bindless({bindless_list_str})] attribute..."),
				);
				error.combine(Error::new(old.span(), "... but two were declared!"));
				return Err(error);
			}
		} else {
			forward.push(arg);
		}
	}

	let mut context = BindlessContext {
		item: &item,
		symbols: &symbols,
		entry_args: TokenStream::new(),
		inner_value: TokenStream::new(),
		inner_arg: TokenStream::new(),
	};

	let _metadata_get = gen_bindless_param(&mut context, bindless_param)?;
	gen_bindless_descriptors(&mut context, bindless_descriptors)?;
	for arg in forward {
		let var_name = &arg.pat;
		quote!(#arg,).to_tokens(&mut context.entry_args);
		quote!(#var_name,).to_tokens(&mut context.inner_value);
		strip_attr(arg).to_tokens(&mut context.inner_arg);
	}

	let crate_ident = &context.symbols.crate_ident;
	let entry_point_attr = TokenStream::from(attr);
	let vis = &context.item.vis;
	let entry_ident = &context.item.sig.ident;
	let entry_args = &context.entry_args;
	let inner_ident = format_ident!("__bindless_{}", entry_ident);
	let inner_value = &context.inner_value;
	let inner_arg = &context.inner_arg;
	let inner_block = &context.item.block;

	// the fn_ident_inner *could* be put within the entry point fn,
	// but putting it outside significantly improves editor performance in rustrover
	Ok(quote! {
		#[#crate_ident::spirv(#entry_point_attr)]
		#[allow(clippy::too_many_arguments)]
		#vis fn #entry_ident(#entry_args) {
			#inner_ident(#inner_value);
		}

		#[allow(clippy::too_many_arguments)]
		fn #inner_ident(#inner_arg) #inner_block
	})
}

fn gen_bindless_descriptors(context: &mut BindlessContext, arg: Option<&PatType>) -> Result<()> {
	if let Some(arg) = arg {
		let crate_ident = &context.symbols.crate_ident;
		let buffers = format_ident!("__bindless_buffers");
		let samplers = format_ident!("__bindless_samplers");

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

		// these "plain" spirv here are correct, as they are non-macro attributes to function arguments, not proc macros!
		context.entry_args.append_tokens(quote! {
			#[spirv(descriptor_set = 0, binding = 0, storage_buffer)] #buffers: &mut #crate_ident::spirv_std::RuntimeArray<[u32]>,
			#image_args
			#[spirv(descriptor_set = 0, binding = 3)] #samplers: &#crate_ident::spirv_std::RuntimeArray<#crate_ident::descriptor::Sampler>,
		});
		context.inner_value.append_tokens(quote! {
			&#crate_ident::descriptor::Descriptors {
				buffers: #buffers,
				#image_values
				samplers: #samplers,
			},
		});
		strip_attr(arg).to_tokens(&mut context.inner_arg);
	}
	Ok(())
}

// Prepared for later
#[allow(unused)]
struct BindlessParam {
	metadata: Ident,
	metadata_extract: TokenStream,
}

fn gen_bindless_param(context: &mut BindlessContext, arg: Option<&PatType>) -> Result<BindlessParam> {
	let crate_ident = &context.symbols.crate_ident;
	// let param_ty = arg.map_or_else(|| quote!(()), |arg| arg.ty.to_token_stream());
	let param_ty = match arg {
		None => Ok(quote!(())),
		Some(arg) => match &*arg.ty {
			Type::Reference(TypeReference {
				mutability: None, elem, ..
			}) => Ok(elem.to_token_stream()),
			_ => Err(Error::new(
				arg.span(),
				"#[bindless(param_constant)] must be taken by reference!",
			)),
		},
	}?;
	let push_constant = format_ident!("__bindless_push_constant");

	// these "plain" spirv here are correct, as they are non-macro attributes to function arguments, not proc macros!
	context.entry_args.append_tokens(quote! {
		#[spirv(push_constant)] #push_constant: &#crate_ident::descriptor::metadata::PushConstant<#param_ty>,
	});
	context.inner_value.append_tokens(quote! {
		&#push_constant.t,
	});
	if let Some(arg) = arg {
		strip_attr(arg).to_tokens(&mut context.inner_arg)
	}

	let metadata = format_ident!("__bindless_metadata");
	Ok(BindlessParam {
		metadata_extract: quote! {
			let #metadata = &#push_constant.metadata;
		},
		metadata,
	})
}

fn strip_attr(arg: &PatType) -> TokenStream {
	let arg = PatType {
		attrs: Vec::new(),
		..arg.clone()
	};
	quote!(#arg,)
}
