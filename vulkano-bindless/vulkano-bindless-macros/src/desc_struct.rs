use crate::symbols::Symbols;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote, ToTokens};
use std::collections::HashSet;
use syn::punctuated::Punctuated;
use syn::visit_mut::VisitMut;
use syn::{visit_mut, Fields, GenericParam, Generics, ItemStruct, Lifetime, Result, Token, TypeParam, TypeParamBound};

pub fn desc_struct(content: proc_macro::TokenStream) -> Result<TokenStream> {
	let symbols = Symbols::new();
	let item = syn::parse::<ItemStruct>(content)?;
	let generics = item
		.generics
		.params
		.iter()
		.filter_map(|g| match g {
			GenericParam::Lifetime(_) => None,
			GenericParam::Type(t) => Some(t.ident.clone()),
			GenericParam::Const(c) => Some(c.ident.clone()),
		})
		.collect();

	let crate_shaders = &symbols.crate_shaders;
	let mut transfer = Punctuated::<TokenStream, Token![,]>::new();
	let mut to = Punctuated::<TokenStream, Token![,]>::new();
	let mut from = Punctuated::<TokenStream, Token![,]>::new();
	let mut gen_name_gen = GenericNameGen::new();
	let mut gen_ref_tys = Vec::new();
	let (transfer, to, from) = match &item.fields {
		Fields::Named(named) => {
			for f in &named.named {
				let name = f.ident.as_ref().unwrap();
				let mut ty = f.ty.clone();
				let mut visitor = GenericsVisitor::new(&generics);
				visit_mut::visit_type_mut(&mut visitor, &mut ty);
				transfer.push(if visitor.found_generics {
					gen_ref_tys.push(f.ty.clone());
					let gen = gen_name_gen.next();
					quote!(#name: #gen)
				} else {
					quote!(#name: <#ty as #crate_shaders::desc_buffer::DescStruct>::TransferDescStruct)
				});
				to.push(quote!(#name: #crate_shaders::desc_buffer::DescStruct::to_transfer(self.#name)));
				from.push(quote!(#name: #crate_shaders::desc_buffer::DescStruct::from_transfer(from.#name, meta)));
			}
			(
				quote!({#transfer}),
				quote!(Self::TransferDescStruct {#to}),
				quote!(Self {#from}),
			)
		}
		Fields::Unnamed(unnamed) => {
			for (i, f) in unnamed.unnamed.iter().enumerate() {
				let mut ty = f.ty.clone();
				let mut visitor = GenericsVisitor::new(&generics);
				visit_mut::visit_type_mut(&mut visitor, &mut ty);
				transfer.push(if visitor.found_generics {
					gen_ref_tys.push(f.ty.clone());
					gen_name_gen.next().into_token_stream()
				} else {
					quote!(<#ty as #crate_shaders::desc_buffer::DescStruct>::TransferDescStruct)
				});
				to.push(quote!(#crate_shaders::desc_buffer::DescStruct::to_transfer(self.#i)));
				from.push(quote!(#crate_shaders::desc_buffer::DescStruct::from_transfer(from.#i, meta)));
			}
			(
				quote!((#transfer)),
				quote!(Self::TransferDescStruct(#to)),
				quote!(Self(#from)),
			)
		}
		Fields::Unit => (
			quote!(;),
			quote!(let _ = self; Self::TransferDescStruct),
			quote!(let _ = (from, meta); Self),
		),
	};

	let generics_decl = Generics {
		params: item
			.generics
			.params
			.iter()
			.map(|param| match param {
				GenericParam::Type(t) => GenericParam::Type(TypeParam {
					bounds: t
						.bounds
						.iter()
						.cloned()
						.chain([TypeParamBound::Verbatim(quote! {
							#crate_shaders::desc_buffer::DescStruct
						})])
						.collect(),
					..t.clone()
				}),
				e => e.clone(),
			})
			.collect(),
		..item.generics.clone()
	};
	let generics_ref = decl_to_ref(item.generics.params.iter());

	let transfer_generics_decl = gen_name_gen.decl(quote! {
		#crate_shaders::bytemuck::AnyBitPattern + Send + Sync
	});
	let transfer_generics_ref = if !gen_ref_tys.is_empty() {
		let gen_ref_tys: Punctuated<TokenStream, Token![,]> = gen_ref_tys
			.into_iter()
			.map(|ty| quote!(<#ty as #crate_shaders::desc_buffer::DescStruct>::TransferDescStruct))
			.collect();
		quote!(<#gen_ref_tys>)
	} else {
		TokenStream::new()
	};

	let vis = &item.vis;
	let ident = &item.ident;
	let transfer_ident = format_ident!("{}Transfer", ident);
	Ok(quote! {
		#[derive(Copy, Clone, #crate_shaders::bytemuck_derive::AnyBitPattern)]
		#vis struct #transfer_ident #transfer_generics_decl #transfer

		unsafe impl #generics_decl #crate_shaders::desc_buffer::DescStruct for #ident #generics_ref {
			type TransferDescStruct = #transfer_ident #transfer_generics_ref;

			unsafe fn to_transfer(self) -> Self::TransferDescStruct {
				#to
			}

			unsafe fn from_transfer(from: Self::TransferDescStruct, meta: #crate_shaders::descriptor::metadata::Metadata) -> Self {
				#from
			}
		}
	})
}

struct GenericsVisitor<'a> {
	generics: &'a HashSet<Ident>,
	found_generics: bool,
}

impl<'a> GenericsVisitor<'a> {
	pub fn new(generics: &'a HashSet<Ident>) -> Self {
		Self {
			generics,
			found_generics: false,
		}
	}
}

impl<'a> VisitMut for GenericsVisitor<'a> {
	fn visit_ident_mut(&mut self, i: &mut Ident) {
		if self.generics.contains(i) {
			self.found_generics = true;
		}
		visit_mut::visit_ident_mut(self, i);
	}

	fn visit_lifetime_mut(&mut self, i: &mut Lifetime) {
		i.ident = Ident::new("static", i.ident.span());
		visit_mut::visit_lifetime_mut(self, i);
	}
}

struct GenericNameGen(u32);

impl GenericNameGen {
	pub fn new() -> Self {
		Self(0)
	}

	pub fn next(&mut self) -> Ident {
		let i = self.0;
		self.0 += 1;
		format_ident!("T{}", i)
	}

	pub fn decl(self, ty: TokenStream) -> Generics {
		let params: Punctuated<GenericParam, Token![,]> = (0..self.0)
			.map(|i| {
				GenericParam::Type(TypeParam {
					attrs: Vec::new(),
					ident: format_ident!("T{}", i),
					colon_token: Some(Default::default()),
					bounds: Punctuated::from_iter([TypeParamBound::Verbatim(ty.clone())]),
					eq_token: None,
					default: None,
				})
			})
			.collect();
		if !params.is_empty() {
			Generics {
				lt_token: Some(Default::default()),
				params,
				gt_token: Some(Default::default()),
				where_clause: None,
			}
		} else {
			Generics::default()
		}
	}
}

fn decl_to_ref<'a>(generics: impl Iterator<Item = &'a GenericParam>) -> TokenStream {
	let out = generics
		.map(|gen| match gen {
			GenericParam::Lifetime(l) => l.lifetime.to_token_stream(),
			GenericParam::Type(t) => t.ident.to_token_stream(),
			GenericParam::Const(c) => c.ident.to_token_stream(),
		})
		.collect::<TokenStream>();
	if out.is_empty() {
		TokenStream::new()
	} else {
		quote!(<#out>)
	}
}
