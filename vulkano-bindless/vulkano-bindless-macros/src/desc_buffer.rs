use crate::symbols::Symbols;
use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::{
	visit_mut, Error, Fields, GenericParam, Generics, ItemStruct, Lifetime, Result, Token, WhereClause, WherePredicate,
};

struct DescBufferContext {
	item: ItemStruct,
	symbols: Symbols,
	lifetime_decl: TokenStream,
	lifetime_ref: TokenStream,
	lifetime_static: TokenStream,
}

pub fn desc_buffer(content: proc_macro::TokenStream) -> Result<TokenStream> {
	let context = {
		let item = syn::parse::<ItemStruct>(content)?;
		let mut lifetime_iter = item.generics.params.iter().filter_map(|gen| {
			if let GenericParam::Lifetime(lifetime) = gen {
				Some(lifetime)
			} else {
				None
			}
		});
		let lifetime = lifetime_iter.next();
		if let Some(b) = lifetime_iter.next() {
			Err(Error::new(b.span(), "DescBuffer must have at most one lifetime!"))?;
		}
		let lifetime_decl = lifetime.as_ref().map_or(TokenStream::new(), |l| quote!(<#l>));
		let lifetime_ref = lifetime.as_ref().map_or(TokenStream::new(), |l| {
			let l = l.lifetime.to_token_stream();
			quote!(<#l>)
		});
		let lifetime_static = lifetime.as_ref().map_or(TokenStream::new(), |_| quote!(<'static>));
		DescBufferContext {
			item,
			symbols: Symbols::new(),
			lifetime_decl,
			lifetime_ref,
			lifetime_static,
		}
	};

	let any_bit_pattern = impl_any_bit_pattern(&context)?;
	let desc_buffer = impl_desc_buffer(&context)?;
	let out = quote! {
		#any_bit_pattern
		#desc_buffer
	};

	Ok(out)
}

fn impl_desc_buffer(context: &DescBufferContext) -> Result<TokenStream> {
	let crate_shaders = &context.symbols.crate_shaders;
	let fields: Punctuated<TokenStream, Token![,]> = match &context.item.fields {
		Fields::Named(named) => named
			.named
			.iter()
			.map(|f| {
				let name = f.ident.as_ref().unwrap();
				quote! {
					#name: #crate_shaders::desc_buffer::DescBuffer::to_static_desc(&self.#name)
				}
			})
			.collect(),
		Fields::Unnamed(unnamed) => (0..unnamed.unnamed.len())
			.map(|i| {
				quote! {
					#crate_shaders::desc_buffer::DescBuffer::to_static_desc(&self.#i)
				}
			})
			.collect(),
		Fields::Unit => Punctuated::new(),
	};

	let ident = &context.item.ident;
	let lifetime_decl = &context.lifetime_decl;
	let lifetime_ref = &context.lifetime_ref;
	let lifetime_static = &context.lifetime_static;
	Ok(quote! {
		unsafe impl #lifetime_decl #crate_shaders::desc_buffer::DescBuffer for #ident #lifetime_ref {
			type DescStatic = #ident #lifetime_static;

			unsafe fn to_static_desc(&self) -> Self::DescStatic {
				Self::DescStatic {
					#fields
				}
			}
		}
	})
}

fn impl_any_bit_pattern(context: &DescBufferContext) -> Result<TokenStream> {
	struct FnVisitor;

	impl VisitMut for FnVisitor {
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

	let mut str_no_lifetime = context.item.clone();
	visit_mut::visit_item_struct_mut(&mut FnVisitor {}, &mut str_no_lifetime);

	let crate_shaders = &context.symbols.crate_shaders;
	let ident = &context.item.ident;
	let lifetime_static = &context.lifetime_static;
	Ok(quote! {
		unsafe impl #crate_shaders::bytemuck::AnyBitPattern for #ident #lifetime_static {}
		unsafe impl #crate_shaders::bytemuck::Zeroable for #ident #lifetime_static {}
		const _: () = {
			#[derive(Copy, Clone, #crate_shaders::bytemuck_derive::AnyBitPattern)]
			#str_no_lifetime
		};
	})
}
