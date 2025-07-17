use crate::symbols::Symbols;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use std::collections::HashSet;
use syn::punctuated::Punctuated;
use syn::visit_mut::VisitMut;
use syn::{
	Fields, GenericParam, Generics, ItemStruct, Lifetime, PathSegment, Result, Token, TypeParam, TypeParamBound,
	visit_mut,
};

pub enum BufferStructType {
	Default,
	Plain,
}

pub fn buffer_struct(ct: BufferStructType, content: proc_macro::TokenStream) -> Result<TokenStream> {
	let symbols = Symbols::new()?;
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

	let crate_buffer_content = match ct {
		BufferStructType::Default => symbols.crate_shaders_buffer_content()?,
		BufferStructType::Plain => symbols.crate_buffer_content.clone(),
	};
	let crate_shader = match ct {
		BufferStructType::Default => symbols.crate_shaders()?.clone(),
		BufferStructType::Plain => format_ident!("you_should_never_see_this_ident"),
	};

	let mut transfer = Punctuated::<TokenStream, Token![,]>::new();
	let mut write_cpu = Punctuated::<TokenStream, Token![,]>::new();
	let mut read = Punctuated::<TokenStream, Token![,]>::new();
	let mut gen_name_gen = GenericNameGen::new();
	let mut gen_ref_tys = Vec::new();
	let (transfer, write_cpu, read) = match &item.fields {
		Fields::Named(named) => {
			for f in &named.named {
				let name = f.ident.as_ref().unwrap();
				let mut ty = f.ty.clone();
				let mut visitor = GenericsVisitor::new(&item.ident, &generics);
				visit_mut::visit_type_mut(&mut visitor, &mut ty);
				transfer.push(if visitor.found_generics {
					gen_ref_tys.push(f.ty.clone());
					let gen_ident = gen_name_gen.next();
					quote!(#name: #gen_ident)
				} else {
					match ct {
						BufferStructType::Default => quote! {
							#name: <#ty as #crate_buffer_content::BufferStruct>::Transfer
						},
						BufferStructType::Plain => quote! {
							#name: <#ty as #crate_buffer_content::BufferStructPlain>::Transfer
						},
					}
				});
				write_cpu.push(match ct {
					BufferStructType::Default => quote! {
						#name: #crate_buffer_content::BufferStruct::write_cpu(self.#name, meta)
					},
					BufferStructType::Plain => quote! {
						#name: #crate_buffer_content::BufferStructPlain::write(self.#name)
					},
				});
				read.push(match ct {
					BufferStructType::Default => quote! {
						#name: #crate_buffer_content::BufferStruct::read(from.#name, meta)
					},
					BufferStructType::Plain => quote! {
						#name: #crate_buffer_content::BufferStructPlain::read(from.#name)
					},
				});
			}
			(
				quote!({#transfer}),
				quote!(Self::Transfer {#write_cpu}),
				quote!(Self {#read}),
			)
		}
		Fields::Unnamed(unnamed) => {
			for (i, f) in unnamed.unnamed.iter().enumerate() {
				let mut ty = f.ty.clone();
				let mut visitor = GenericsVisitor::new(&item.ident, &generics);
				visit_mut::visit_type_mut(&mut visitor, &mut ty);
				transfer.push(if visitor.found_generics {
					gen_ref_tys.push(f.ty.clone());
					gen_name_gen.next().into_token_stream()
				} else {
					match ct {
						BufferStructType::Default => quote! {
							<#ty as #crate_buffer_content::BufferStruct>::Transfer
						},
						BufferStructType::Plain => quote! {
							<#ty as #crate_buffer_content::BufferStructPlain>::Transfer
						},
					}
				});
				let index = syn::Index::from(i);
				write_cpu.push(match ct {
					BufferStructType::Default => quote! {
						#index: #crate_buffer_content::BufferStruct::write_cpu(self.#index, meta)
					},
					BufferStructType::Plain => quote! {
						#index: #crate_buffer_content::BufferStructPlain::write(self.#index)
					},
				});
				read.push(match ct {
					BufferStructType::Default => quote! {
						#crate_buffer_content::BufferStruct::read(from.#index, meta)
					},
					BufferStructType::Plain => quote! {
						#crate_buffer_content::BufferStructPlain::read(from.#index)
					},
				});
			}
			(
				quote!((#transfer);),
				quote!(Self::Transfer { #write_cpu }),
				quote!(Self(#read)),
			)
		}
		Fields::Unit => match ct {
			BufferStructType::Default => (
				quote!(;),
				quote!(let _ = (self, meta); Self::Transfer {}),
				quote!(let _ = (from, meta); Self),
			),
			BufferStructType::Plain => (
				quote!(;),
				quote!(let _ = self; Self::Transfer {}),
				quote!(let _ = from; Self),
			),
		},
	};

	let generics_decl = &item.generics;
	let generics_ref = decl_to_ref(item.generics.params.iter());
	let generics_where = gen_ref_tys
		.iter()
		.map(|ty| match ct {
			BufferStructType::Default => quote!(#ty: #crate_buffer_content::BufferStruct),
			BufferStructType::Plain => quote!(#ty: #crate_buffer_content::BufferStructPlain),
		})
		.collect::<Punctuated<TokenStream, Token![,]>>()
		.into_token_stream();

	let transfer_generics_decl = gen_name_gen.decl(match ct {
		BufferStructType::Default => quote! {
			#crate_shader::__private::bytemuck::AnyBitPattern + Send + Sync
		},
		BufferStructType::Plain => quote! {
			#crate_buffer_content::__private::bytemuck::AnyBitPattern + Send + Sync
		},
	});
	let transfer_generics_ref = gen_ref_tys
		.iter()
		.map(|ty| match ct {
			BufferStructType::Default => quote!(<#ty as #crate_buffer_content::BufferStruct>::Transfer),
			BufferStructType::Plain => quote!(<#ty as #crate_buffer_content::BufferStructPlain>::Transfer),
		})
		.collect::<Punctuated<TokenStream, Token![,]>>()
		.into_token_stream();

	let vis = &item.vis;
	let ident = &item.ident;
	let transfer_ident = format_ident!("{}Transfer", ident);
	Ok(match ct {
		BufferStructType::Default => quote! {
			#[derive(Copy, Clone, #crate_shader::__private::bytemuck_derive::AnyBitPattern)]
			#vis struct #transfer_ident #transfer_generics_decl #transfer

			unsafe impl #generics_decl #crate_buffer_content::BufferStruct for #ident #generics_ref
			where
				#ident #generics_ref: Copy,
				#generics_where
			{
				type Transfer = #transfer_ident <#transfer_generics_ref>;

				unsafe fn write_cpu(self, meta: &mut impl #crate_buffer_content::MetadataCpuInterface) -> Self::Transfer {
					#write_cpu
				}

				unsafe fn read(from: Self::Transfer, meta: #crate_buffer_content::Metadata) -> Self {
					#read
				}
			}
		},
		BufferStructType::Plain => quote! {
			#[derive(Copy, Clone, #crate_buffer_content::__private::bytemuck_derive::AnyBitPattern)]
			#vis struct #transfer_ident #transfer_generics_decl #transfer

			unsafe impl #generics_decl #crate_buffer_content::BufferStructPlain for #ident #generics_ref
			where
				#ident #generics_ref: Copy,
				#generics_where
			{
				type Transfer = #transfer_ident <#transfer_generics_ref>;

				unsafe fn write(self) -> Self::Transfer {
					#write_cpu
				}

				unsafe fn read(from: Self::Transfer) -> Self {
					#read
				}
			}
		},
	})
}

struct GenericsVisitor<'a> {
	self_ident: &'a Ident,
	generics: &'a HashSet<Ident>,
	found_generics: bool,
}

impl<'a> GenericsVisitor<'a> {
	pub fn new(self_ident: &'a Ident, generics: &'a HashSet<Ident>) -> Self {
		Self {
			self_ident,
			generics,
			found_generics: false,
		}
	}
}

impl VisitMut for GenericsVisitor<'_> {
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

	fn visit_path_segment_mut(&mut self, i: &mut PathSegment) {
		if i.ident == Ident::new("Self", Span::call_site()) {
			i.ident = self.self_ident.clone();
		}
		visit_mut::visit_path_segment_mut(self, i);
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
		.map(|generic| match generic {
			GenericParam::Lifetime(l) => l.lifetime.to_token_stream(),
			GenericParam::Type(t) => t.ident.to_token_stream(),
			GenericParam::Const(c) => c.ident.to_token_stream(),
		})
		.collect::<Punctuated<TokenStream, Token![,]>>();
	if out.is_empty() {
		TokenStream::new()
	} else {
		quote!(<#out>)
	}
}
