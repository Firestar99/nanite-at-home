use crate::AppendTokens;
use crate::image_types::standard_image_types;
use crate::symbols::Symbols;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::spanned::Spanned;
use syn::{Error, FnArg, ItemFn, MetaList, PatType, Result, ReturnType, Type, TypeReference};

pub struct BindlessContext<'a> {
	symbols: &'a Symbols,
	item: &'a ItemFn,
	attr: &'a MetaList,
	entry_args: TokenStream,
	entry_content: TokenStream,
}

pub fn bindless(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> Result<TokenStream> {
	let symbols = Symbols::new()?;
	let item = syn::parse::<ItemFn>(item)?;
	let attr = syn::parse::<MetaList>(attr)?;
	match &item.sig.output {
		ReturnType::Default => (),
		ReturnType::Type(_, e) => return Err(Error::new(e.span(), "Entry points must not return anything!")),
	}

	let mut arg_param = None;
	let mut arg_descriptors = None;
	let mut forward = Vec::new();
	for arg in item.sig.inputs.iter() {
		let arg = match arg {
			FnArg::Receiver(e) => {
				return Err(Error::new(
					e.span(),
					"Entry points may not contain a receiver (eg. self) argument!",
				));
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
				"param" => &mut arg_param,
				"descriptors" => &mut arg_descriptors,
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
		entry_content: TokenStream::new(),
	};

	let push_constant = gen_bindless_push_constant(&mut context, arg_param)?;
	let descriptors = gen_bindless_descriptors(&mut context)?;
	let inner_call = gen_bindless_inner_call(
		&mut context,
		&push_constant,
		&descriptors,
		arg_param,
		arg_descriptors,
		&forward,
	)?;
	let entry_shader_type = get_entry_shader_type(&mut context)?;

	let entry_ident = &context.item.sig.ident;
	// same formatting in macros and shader-builder
	let entry_shader_type_ident = format_ident!("__Bindless_{}_ShaderType", entry_ident);
	let param_type_ident = format_ident!("__Bindless_{}_ParamConstant", entry_ident);
	let param_type = &push_constant.param_ty;

	let crate_shaders = &context.symbols.crate_shaders()?;
	let vis = &context.item.vis;
	let entry_args = &context.entry_args;
	let entry_content = &context.entry_content;
	let inner_ident = format_ident!("__bindless_{}", entry_ident);
	let inner_params = &inner_call.params;
	let inner_args = &inner_call.args;
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
			#entry_content
			#inner_ident(#inner_params);
		}

		#[allow(clippy::too_many_arguments)]
		fn #inner_ident(#inner_args) #inner_block
	})
}

#[allow(unused)]
struct SymPushConstant {
	push_constant: TokenStream,
	param_ty: TokenStream,
}

fn gen_bindless_push_constant(
	context: &mut BindlessContext,
	bindless_param: Option<&PatType>,
) -> Result<SymPushConstant> {
	let crate_shaders = &context.symbols.crate_shaders()?;
	let param_ty = match bindless_param {
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
		#[spirv(push_constant)] #push_constant: &#crate_shaders::descriptor::BindlessPushConstant,
	});
	Ok(SymPushConstant {
		push_constant: push_constant.into_token_stream(),
		param_ty,
	})
}

#[allow(unused)]
struct SymDescriptors {
	descriptors: TokenStream,
}

fn gen_bindless_descriptors(context: &mut BindlessContext) -> Result<SymDescriptors> {
	let crate_shaders = &context.symbols.crate_shaders()?;
	let buffers = format_ident!("__bindless_buffers");
	let buffers_mut = format_ident!("__bindless_buffers_mut");
	let samplers = format_ident!("__bindless_samplers");
	let descriptors = format_ident!("__bindless_descriptors");

	let image_args;
	let image_values;
	macro_rules! make_image_args {
		($($image:ident: $sampled:ident $storage:ident,)*) => {
			$(let $storage = format_ident!("__bindless_{}", stringify!($storage));)*
			$(let $sampled = format_ident!("__bindless_{}", stringify!($sampled));)*

			image_args = quote! {
				$(#[spirv(descriptor_set = 0, binding = 1)] #$storage: &#crate_shaders::spirv_std::RuntimeArray<<#crate_shaders::descriptor::$image as #crate_shaders::descriptor::ImageType>::StorageSpvImage>,)*
				$(#[spirv(descriptor_set = 0, binding = 2)] #$sampled: &#crate_shaders::spirv_std::RuntimeArray<<#crate_shaders::descriptor::$image as #crate_shaders::descriptor::ImageType>::SampledSpvImage>,)*
			};
			image_values = quote! {
				$($storage: #$storage,)*
				$($sampled: #$sampled,)*
			};
		};
	}
	standard_image_types!(make_image_args);

	// these "plain" spirv here are correct, as they are non-macro attributes to function arguments, not proc macros!
	context.entry_args.append_tokens(quote! {
			#[spirv(descriptor_set = 0, binding = 0, storage_buffer)] #buffers: &#crate_shaders::spirv_std::RuntimeArray<#crate_shaders::spirv_std::TypedBuffer<[u32]>>,
			#[spirv(descriptor_set = 0, binding = 0, storage_buffer)] #buffers_mut: &mut #crate_shaders::spirv_std::RuntimeArray<#crate_shaders::spirv_std::TypedBuffer<[u32]>>,
			#image_args
			#[spirv(descriptor_set = 0, binding = 3)] #samplers: &#crate_shaders::spirv_std::RuntimeArray<#crate_shaders::descriptor::Sampler>,
		});
	context.entry_content.append_tokens(quote! {
		let #descriptors = #crate_shaders::descriptor::Descriptors {
			buffers: #buffers,
			buffers_mut: #buffers_mut,
			#image_values
			samplers: #samplers,
			meta: #crate_shaders::buffer_content::Metadata {},
		};
	});
	Ok(SymDescriptors {
		descriptors: descriptors.into_token_stream(),
	})
}

struct SymInnerCall {
	params: TokenStream,
	args: TokenStream,
}

fn gen_bindless_inner_call(
	context: &mut BindlessContext,
	push_constant: &SymPushConstant,
	descriptors: &SymDescriptors,
	arg_param: Option<&PatType>,
	arg_descriptors: Option<&PatType>,
	forward: &[&PatType],
) -> Result<SymInnerCall> {
	let mut params = TokenStream::new();
	let mut args = TokenStream::new();

	if let Some(arg) = arg_descriptors {
		let descriptors = &descriptors.descriptors;
		params.append_tokens(quote!(#descriptors,));
		args.append_tokens(strip_attr(arg));
	}
	if let Some(arg) = arg_param {
		let param_ty = &push_constant.param_ty;
		let push_constant = &push_constant.push_constant;
		let descriptors = &descriptors.descriptors;
		let param = format_ident!("__bindless_param");
		context.entry_content.append_tokens(quote! {
			let #param = #push_constant.load_param::<#param_ty>(&#descriptors);
		});
		params.append_tokens(quote!(&#param,));
		args.append_tokens(strip_attr(arg));
	}
	for arg in forward {
		let var_name = &arg.pat;
		quote!(#arg,).to_tokens(&mut context.entry_args);
		quote!(#var_name,).to_tokens(&mut params);
		strip_attr(arg).to_tokens(&mut args);
	}
	Ok(SymInnerCall { params, args })
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
	let crate_shaders = &context.symbols.crate_shaders()?;
	Ok(quote!(#crate_shaders::shader_type::#shader_type))
}
