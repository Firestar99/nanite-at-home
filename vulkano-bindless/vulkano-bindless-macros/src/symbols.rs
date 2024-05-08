use proc_macro2::Ident;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::format_ident;

pub struct Symbols {
	pub crate_ident: Ident,
	pub bindless: Ident,
}

impl Symbols {
	pub fn new() -> Self {
		let crate_ident = crate_ident();
		Self {
			bindless: format_ident!("bindless"),
			crate_ident,
		}
	}
}

fn crate_ident() -> Ident {
	let found_crate = crate_name("vulkano-bindless-shaders").unwrap();
	let name = match &found_crate {
		FoundCrate::Itself => "vulkano-bindless-shaders",
		FoundCrate::Name(name) => name,
	};
	Ident::new(name, proc_macro2::Span::call_site())
}
