use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Ident;
use quote::format_ident;
use syn::{Path, PathSegment, Token, punctuated};

pub struct Symbols {
	pub bindless: Ident,
	pub ident_crate: Ident,
	pub crate_buffer_content: Path,
	crate_shaders: Result<Ident, syn::Error>,
}

impl Symbols {
	pub fn new() -> Result<Self, syn::Error> {
		let span = proc_macro2::Span::call_site();
		let ident_crate = format_ident!("crate");

		let crate_shaders = match crate_name("rust-gpu-bindless-shaders") {
			Ok(found_crate) => Ok(match &found_crate {
				FoundCrate::Itself => ident_crate.clone(),
				FoundCrate::Name(name) => format_ident!("{}", name),
			}),
			Err(err) => Err(err),
		};

		let crate_buffer_content = if let Ok(crate_shaders_ident) = &crate_shaders {
			idents_to_path(&ident_crate, &[crate_shaders_ident, &format_ident!("buffer_content")])
		} else {
			match crate_name("rust-gpu-bindless-buffer-content") {
				Ok(found_crate) => match &found_crate {
					FoundCrate::Itself => idents_to_path(&ident_crate, &[&ident_crate]),
					FoundCrate::Name(name) => idents_to_path(&ident_crate, &[&format_ident!("{}", name)]),
				},
				Err(err) => {
					return Err(syn::Error::new(span, err));
				}
			}
		};

		let crate_shaders = crate_shaders.map_err(|e| syn::Error::new(span, e));
		Ok(Self {
			bindless: format_ident!("bindless"),
			ident_crate,
			crate_shaders,
			crate_buffer_content,
		})
	}

	pub fn crate_shaders(&self) -> Result<&Ident, syn::Error> {
		self.crate_shaders.as_ref().map_err(Clone::clone)
	}

	pub fn crate_shaders_buffer_content(&self) -> Result<Path, syn::Error> {
		Ok(idents_to_path(
			&self.ident_crate,
			&[self.crate_shaders()?, &format_ident!("buffer_content")],
		))
	}
}

fn idents_to_path(ident_crate: &Ident, idents: &[&Ident]) -> Path {
	let mut idents = idents.iter().peekable();
	Path {
		leading_colon: idents
			.peek()
			.map(|i| **i != ident_crate)
			.unwrap()
			.then(|| Token![::](proc_macro2::Span::call_site())),
		segments: punctuated::Punctuated::from_iter(idents.map(|i| PathSegment::from((*i).clone()))),
	}
}
