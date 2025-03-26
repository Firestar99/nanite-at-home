use proc_macro_crate::FoundCrate;
use quote::format_ident;
use std::fmt::{Debug, Display, Formatter};
use thiserror::Error;

pub fn find_rust_gpu_bindless() -> Result<syn::Ident, FindRustGpuBindlessError> {
	for crate_name in ["rust-gpu-bindless", "rust-gpu-bindless-core"] {
		match proc_macro_crate::crate_name(crate_name) {
			Ok(found_crate) => {
				let name = match &found_crate {
					FoundCrate::Itself => crate_name,
					FoundCrate::Name(name) => name,
				};
				return Ok(format_ident!("{}", name));
			}
			Err(proc_macro_crate::Error::CrateNotFound { .. }) => (),
			Err(err) => return Err(FindRustGpuBindlessError::ProcMacroCrateError(err)),
		}
	}
	Err(FindRustGpuBindlessError::RustGpuBindlessNotFound)
}

#[derive(Error)]
pub enum FindRustGpuBindlessError {
	#[error("Found neither `rust-gpu-bindless` nor `rust-gpu-bindless-core` in `dependencies`")]
	RustGpuBindlessNotFound,
	#[error("proc_macro_crate::Error: {0}")]
	ProcMacroCrateError(#[from] proc_macro_crate::Error),
}

impl Debug for FindRustGpuBindlessError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(self, f)
	}
}
