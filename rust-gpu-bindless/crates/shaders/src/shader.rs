use crate::buffer_content::BufferStruct;
use crate::shader_type::ShaderType;
use core::ffi::CStr;

pub trait BindlessShader {
	type ShaderType: ShaderType;
	type ParamConstant: BufferStruct;

	/// Get the spirv binary and the entry point name.
	/// Currently, `&self` isn't really necessary as it would always be returning `SpirvBinary<'static>`, but it makes
	/// it easier to work with.
	fn spirv_binary(&self) -> &SpirvBinary<'_>;
}

pub struct SpirvBinary<'a> {
	pub binary: &'a [u32],
	pub entry_point_name: &'a CStr,
}
