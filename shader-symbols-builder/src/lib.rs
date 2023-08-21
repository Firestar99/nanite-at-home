use error::Error;
use std::{env, error};
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use spirv_builder::{CompileResult, MetadataPrintout, ModuleResult, SpirvBuilder, SpirvBuilderError, SpirvMetadata};

use crate::codegen::{codegen_shader_symbols, CodegenError, CodegenOptions};

pub mod codegen;

pub struct ShaderSymbolsBuilder {
	pub spirv_builder: SpirvBuilder,
	pub codegen: Option<CodegenOptions>,
}

impl ShaderSymbolsBuilder {
	pub fn new(relative_crate: &str, target: impl Into<String>) -> Self {
		let manifest_dir = env!("CARGO_MANIFEST_DIR");

		let crate_path = [manifest_dir, "..", relative_crate].iter().copied().collect::<PathBuf>();
		ShaderSymbolsBuilder::new_absolute_path(crate_path, target)
	}

	pub fn new_absolute_path(path_to_crate: impl AsRef<Path>, target: impl Into<String>) -> Self {
		Self {
			spirv_builder: SpirvBuilder::new(path_to_crate, target)
				// we want multiple *.spv files for vulkano's shader! macro to only generate needed structs
				.multimodule(true)
				// this needs at least NameVariables for vulkano to like the spv, but may also be Full
				.spirv_metadata(SpirvMetadata::NameVariables)
				// has to be DependencyOnly!
				// may not be None as it's needed for cargo
				// may not be Full as that's unsupported with multimodule
				.print_metadata(MetadataPrintout::DependencyOnly),
			codegen: Some(CodegenOptions {
				shader_symbols_path: String::from("shader_symbols.rs")
			}),
		}
	}

	pub fn build(self) -> Result<ShaderSymbolsResult, ShaderSymbolsError> {
		let spirv_result = self.spirv_builder.build().map_err(|o| ShaderSymbolsError::SpirvBuilderError(o))?;
		let codegen_out_path = if let Some(codegen) = &self.codegen {
			let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join(&codegen.shader_symbols_path);
			match &spirv_result.module {
				ModuleResult::SingleModule(path) => {
					codegen_shader_symbols(spirv_result.entry_points.iter().map(|name| (name.as_str(), path)), &out_path, codegen)
				}
				ModuleResult::MultiModule(m) => {
					codegen_shader_symbols(m.iter().map(|(name, path)| (name.as_str(), path)), &out_path, codegen)
				}
			}.map_err(|e| ShaderSymbolsError::CodegenError(e))?;
			Some(out_path)
		} else {
			None
		};
		Ok(ShaderSymbolsResult {
			codegen_out_path,
			spirv_result,
		})
	}
}

pub struct ShaderSymbolsResult {
	pub spirv_result: CompileResult,
	pub codegen_out_path: Option<PathBuf>,
}

#[derive(Debug)]
pub enum ShaderSymbolsError {
	SpirvBuilderError(SpirvBuilderError),
	CodegenError(CodegenError),
}

impl Display for ShaderSymbolsError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			ShaderSymbolsError::SpirvBuilderError(e) => {
				write!(f, "SpirvBuilder error: {}", e)
			}
			ShaderSymbolsError::CodegenError(e) => {
				write!(f, "Codegen error: {}", e)
			}
		}
	}
}

impl Error for ShaderSymbolsError {

}
