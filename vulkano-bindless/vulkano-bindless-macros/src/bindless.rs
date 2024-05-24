use crate::image_types::standard_image_types;
use crate::symbols::Symbols;
use crate::AppendTokens;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::spanned::Spanned;
use syn::{Error, FnArg, ItemFn, MetaList, PatType, Result, ReturnType, Type, TypeReference};

pub struct BindlessContext<'a> {
	symbols: &'a Symbols,
	item: &'a ItemFn,
	attr: &'a MetaList,
	entry_args: TokenStream,
	inner_value: TokenStream,
	inner_arg: TokenStream,
}

pub fn bindless(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> Result<TokenStream> {
	let symbols = Symbols::new();
	let item = syn::parse::<ItemFn>(item)?;
	let attr = syn::parse::<MetaList>(attr)?;
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
		symbols: &symbols,
		item: &item,
		attr: &attr,
		entry_args: TokenStream::new(),
		inner_value: TokenStream::new(),
		inner_arg: TokenStream::new(),
	};

	let param = gen_bindless_param(&mut context, bindless_param)?;
	gen_bindless_descriptors(&mut context, bindless_descriptors, &param)?;
	for arg in forward {
		let var_name = &arg.pat;
		quote!(#arg,).to_tokens(&mut context.entry_args);
		quote!(#var_name,).to_tokens(&mut context.inner_value);
		strip_attr(arg).to_tokens(&mut context.inner_arg);
	}
	let entry_shader_type = get_entry_shader_type(&mut context)?;

	let entry_ident = &context.item.sig.ident;
	// same formatting in macros and shader-builder
	let entry_shader_type_ident = format_ident!("__Bindless_{}_ShaderType", entry_ident);
	let param_type_ident = format_ident!("__Bindless_{}_ParamConstant", entry_ident);
	let param_type = &param.param_ty;

	let crate_shaders = &context.symbols.crate_shaders;
	let vis = &context.item.vis;
	let entry_args = &context.entry_args;
	let inner_ident = format_ident!("__bindless_{}", entry_ident);
	let inner_value = &context.inner_value;
	let inner_arg = &context.inner_arg;
	let inner_block = &context.item.block;

	// the fn_ident_inner *could* be put within the entry point fn,
	// but putting it outside significantly improves editor performance in rustrover
	Ok(quote! {
		#[allow(non_camel_case_types)]
		#vis type #entry_shader_type_ident = #entry_shader_type;
		#[allow(non_camel_case_types)]
		#vis type #param_type_ident = #param_type;

		#[#crate_shaders::spirv(#attr)]
		#[allow(clippy::too_many_arguments)]
		#vis fn #entry_ident(#entry_args) {
			#inner_ident(#inner_value);
		}

		#[allow(clippy::too_many_arguments)]
		fn #inner_ident(#inner_arg) #inner_block
	})
}

fn gen_bindless_descriptors(context: &mut BindlessContext, arg: Option<&PatType>, param: &BindlessParam) -> Result<()> {
	if let Some(arg) = arg {
		let crate_shaders = &context.symbols.crate_shaders;
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
					$(#[spirv(descriptor_set = 0, binding = 1)] #$storage_name: &#crate_shaders::spirv_std::RuntimeArray<#crate_shaders$storage_ty>,)*
					$(#[spirv(descriptor_set = 0, binding = 2)] #$sampled_name: &#crate_shaders::spirv_std::RuntimeArray<#crate_shaders$sampled_ty>,)*
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
			#[spirv(descriptor_set = 0, binding = 0, storage_buffer)] #buffers: &mut #crate_shaders::spirv_std::RuntimeArray<[u32]>,
			#image_args
			#[spirv(descriptor_set = 0, binding = 3)] #samplers: &#crate_shaders::spirv_std::RuntimeArray<#crate_shaders::descriptor::Sampler>,
		});
		let _meta = &param.metadata;
		context.inner_value.append_tokens(quote! {
			&#crate_shaders::descriptor::Descriptors {
				buffers: #buffers,
				#image_values
				samplers: #samplers,
				meta: #_meta,
			},
		});
		strip_attr(arg).to_tokens(&mut context.inner_arg);
	}
	Ok(())
}

// Prepared for later
#[allow(unused)]
struct BindlessParam {
	metadata: TokenStream,
	param_ty: TokenStream,
}

fn gen_bindless_param(context: &mut BindlessContext, arg: Option<&PatType>) -> Result<BindlessParam> {
	let crate_shaders = &context.symbols.crate_shaders;
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
		#[spirv(push_constant)] #push_constant: &#crate_shaders::descriptor::metadata::PushConstant<#param_ty>,
	});
	context.inner_value.append_tokens(quote! {
		&#push_constant.t,
	});
	if let Some(arg) = arg {
		strip_attr(arg).to_tokens(&mut context.inner_arg)
	}

	Ok(BindlessParam {
		metadata: quote!(#push_constant.metadata),
		param_ty,
	})
}

fn strip_attr(arg: &PatType) -> TokenStream {
	let arg = PatType {
		attrs: Vec::new(),
		..arg.clone()
	};
	quote!(#arg,)
}

fn get_entry_shader_type(context: &mut BindlessContext) -> Result<TokenStream> {
	let attr = context.attr;
	let shader_type = attr
		.path
		.get_ident()
		.ok_or_else(|| Error::new(attr.path.span(), "entry point type is not an ident"))?;
	let shader_type_name = match shader_type.to_string().as_str() {
		"vertex" => "VertexShader",
		"tessellation_control" => "TessellationControlShader",
		"tessellation_evaluation" => "TessellationEvaluationShader",
		"geometry" => "GeometryShader",
		"fragment" => "FragmentShader",
		"compute" => "ComputeShader",
		"task_ext" => "TaskShader",
		"mesh_ext" => "MeshShader",
		_ => Err(Error::new(attr.path.span(), "Unknown bindless shader type"))?,
	};
	let shader_type = format_ident!("{}", shader_type_name);
	let crate_shaders = &context.symbols.crate_shaders;
	Ok(quote!(#crate_shaders::shader_type::#shader_type))
}
