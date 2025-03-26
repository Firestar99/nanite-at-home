use crate::codegen::{codegen_shader_symbols, CodegenOptions};
use proc_macro_crate::FoundCrate;
use spirv_builder::{
	Capability, CompileResult, MetadataPrintout, ModuleResult, ShaderPanicStrategy, SpirvBuilder, SpirvMetadata,
};
use std::env;
use std::path::{Path, PathBuf};

pub mod codegen;

pub use spirv_builder;

pub struct ShaderSymbolsBuilder {
	spirv_builder: SpirvBuilder,
	pub codegen: Option<CodegenOptions>,
	pub crate_name: String,
}

impl ShaderSymbolsBuilder {
	/// Build the shader crate named `crate_name` at path `../{crate_name}` relative to your `Cargo.toml`. If your crate
	/// name and crate path do not match, use the [`Self::new_relative_path`] or [`Self::new_absolute_path`] functions
	/// instead.
	pub fn new(crate_name: &str, target: impl Into<String>) -> Self {
		let found_crate = proc_macro_crate::crate_name(crate_name).unwrap();
		let crate_ident = match &found_crate {
			FoundCrate::Itself => crate_name,
			FoundCrate::Name(name) => name,
		};
		ShaderSymbolsBuilder::new_relative_path(crate_name, crate_ident, target)
	}

	/// Build the shader crate at path `../{relative_crate_path}` relative to your `Cargo.toml` and assume the crate is
	/// accessible with the ident `crate_ident`.
	pub fn new_relative_path(relative_crate_path: &str, crate_ident: &str, target: impl Into<String>) -> Self {
		let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
		let crate_path = [&manifest_dir, "..", relative_crate_path]
			.iter()
			.copied()
			.collect::<PathBuf>();
		ShaderSymbolsBuilder::new_absolute_path(crate_path, crate_ident, target)
	}

	/// Build the shader crate at path `absolute_crate_path` and assume the crate is
	/// accessible with the ident `crate_ident`.
	pub fn new_absolute_path(
		absolute_crate_path: impl AsRef<Path>,
		crate_ident: &str,
		target: impl Into<String>,
	) -> Self {
		Self {
			spirv_builder: SpirvBuilder::new(absolute_crate_path, target)
				// we want multiple *.spv files for vulkano's shader! macro to only generate needed structs
				.multimodule(true)
				// this needs at least NameVariables for vulkano to like the spv, but may also be Full
				.spirv_metadata(SpirvMetadata::NameVariables)
				// has to be DependencyOnly!
				// may not be None as it's needed for cargo
				// may not be Full as that's unsupported with multimodule
				.print_metadata(MetadataPrintout::DependencyOnly)
				// required capabilities
				.capability(Capability::RuntimeDescriptorArray)
				.capability(Capability::ShaderNonUniform)
				.capability(Capability::StorageBufferArrayDynamicIndexing)
				.capability(Capability::StorageImageArrayDynamicIndexing)
				.capability(Capability::SampledImageArrayDynamicIndexing)
				.capability(Capability::StorageBufferArrayNonUniformIndexing)
				.capability(Capability::StorageImageArrayNonUniformIndexing)
				.capability(Capability::SampledImageArrayNonUniformIndexing),
			codegen: Some(CodegenOptions {
				shader_symbols_path: String::from("shader_symbols.rs"),
			}),
			crate_name: String::from(crate_ident),
		}
	}

	pub fn with_spirv_builder<F>(self, f: F) -> Self
	where
		F: FnOnce(SpirvBuilder) -> SpirvBuilder,
	{
		Self {
			spirv_builder: f(self.spirv_builder),
			..self
		}
	}

	pub fn extension(self, extension: impl Into<String>) -> Self {
		Self {
			spirv_builder: self.spirv_builder.extension(extension),
			..self
		}
	}

	pub fn capability(self, capability: Capability) -> Self {
		Self {
			spirv_builder: self.spirv_builder.capability(capability),
			..self
		}
	}

	pub fn spirv_metadata(self, v: SpirvMetadata) -> Self {
		assert_ne!(
			v,
			SpirvMetadata::None,
			"SpirvMetadata must not be None. Vulkano `shader!` macros need at least NameVariables"
		);
		Self {
			spirv_builder: self.spirv_builder.spirv_metadata(v),
			..self
		}
	}

	pub fn shader_panic_strategy(self, v: ShaderPanicStrategy) -> Self {
		Self {
			spirv_builder: self.spirv_builder.shader_panic_strategy(v),
			..self
		}
	}

	pub fn set_codegen_options(self, codegen: Option<CodegenOptions>) -> Self {
		Self { codegen, ..self }
	}

	pub fn build(self) -> anyhow::Result<ShaderSymbolsResult> {
		let spirv_result = self.spirv_builder.build()?;
		let codegen_out_path = if let Some(codegen) = &self.codegen {
			let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join(&codegen.shader_symbols_path);
			match &spirv_result.module {
				ModuleResult::SingleModule(path) => codegen_shader_symbols(
					spirv_result.entry_points.iter().map(|name| (name.as_str(), path)),
					&self.crate_name,
					&out_path,
					codegen,
				),
				ModuleResult::MultiModule(m) => codegen_shader_symbols(
					m.iter().map(|(name, path)| (name.as_str(), path)),
					&self.crate_name,
					&out_path,
					codegen,
				),
			}?;
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
