use crate::symbols::Symbols;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::visit_mut::VisitMut;
use syn::{
	visit_mut, Fields, GenericParam, Generics, ItemStruct, Lifetime, Result, Token, WhereClause, WherePredicate,
};

struct DescBufferContext {
	item: ItemStruct,
	symbols: Symbols,
	generics_decl: TokenStream,
	generics_ref: TokenStream,
	transfer_generics_decl: TokenStream,
	transfer_generics_ref: TokenStream,
}

pub fn desc_struct(content: proc_macro::TokenStream) -> Result<TokenStream> {
	let context = {
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

		let item = syn::parse::<ItemStruct>(content)?;
		let transfer_generics = Generics {
			params: item
				.generics
				.params
				.iter()
				.filter(|gen| !matches!(gen, GenericParam::Lifetime(_)))
				.cloned()
				.collect(),
			where_clause: item.generics.where_clause.as_ref().map(|wh| WhereClause {
				predicates: wh
					.predicates
					.iter()
					.filter(|pred| !matches!(pred, WherePredicate::Lifetime(_)))
					.cloned()
					.collect(),
				..wh.clone()
			}),
			..item.generics.clone()
		};
		DescBufferContext {
			symbols: Symbols::new(),
			generics_decl: item.generics.to_token_stream(),
			generics_ref: decl_to_ref(item.generics.params.iter()),
			transfer_generics_decl: transfer_generics.to_token_stream(),
			transfer_generics_ref: decl_to_ref(transfer_generics.params.iter()),
			item,
		}
	};

	let desc_buffer = impl_desc_buffer(&context)?;
	let out = quote! {
		#desc_buffer
	};

	Ok(out)
}

fn impl_desc_buffer(context: &DescBufferContext) -> Result<TokenStream> {
	let crate_shaders = &context.symbols.crate_shaders;
	let (transfer, to, from) = match &context.item.fields {
		Fields::Named(named) => {
			let mut transfer = Punctuated::<TokenStream, Token![,]>::new();
			let mut to = Punctuated::<TokenStream, Token![,]>::new();
			let mut from = Punctuated::<TokenStream, Token![,]>::new();
			for f in &named.named {
				let name = f.ident.as_ref().unwrap();
				let mut ty = f.ty.clone();
				visit_mut::visit_type_mut(&mut RemoveLifetimesVisitor, &mut ty);
				transfer.push(quote!(#name: <#ty as #crate_shaders::desc_buffer::DescStruct>::TransferDescStruct));
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
			let mut transfer = Punctuated::<TokenStream, Token![,]>::new();
			let mut to = Punctuated::<TokenStream, Token![,]>::new();
			let mut from = Punctuated::<TokenStream, Token![,]>::new();
			for (i, f) in unnamed.unnamed.iter().enumerate() {
				let mut ty = f.ty.clone();
				visit_mut::visit_type_mut(&mut RemoveLifetimesVisitor, &mut ty);
				transfer.push(quote!(<#ty as #crate_shaders::desc_buffer::DescStruct>::TransferDescStruct));
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

	let vis = &context.item.vis;
	let ident = &context.item.ident;
	let transfer_ident = format_ident!("{}Transfer", ident);
	let generics_decl = &context.generics_decl;
	let generics_ref = &context.generics_ref;
	let transfer_generics_decl = &context.transfer_generics_decl;
	let transfer_generics_ref = &context.transfer_generics_ref;
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

struct RemoveLifetimesVisitor;

impl VisitMut for RemoveLifetimesVisitor {
	fn visit_generics_mut(&mut self, i: &mut Generics) {
		i.params = i
			.params
			.iter()
			.filter(|gen| !matches!(gen, GenericParam::Lifetime(_)))
			.cloned()
			.collect();
		i.where_clause = i.where_clause.as_ref().map(|wh| WhereClause {
			predicates: wh
				.predicates
				.iter()
				.filter(|pred| !matches!(pred, WherePredicate::Lifetime(_)))
				.cloned()
				.collect(),
			..wh.clone()
		});
		visit_mut::visit_generics_mut(self, i);
	}

	fn visit_lifetime_mut(&mut self, i: &mut Lifetime) {
		i.ident = Ident::new("static", i.ident.span());
		visit_mut::visit_lifetime_mut(self, i);
	}
}
