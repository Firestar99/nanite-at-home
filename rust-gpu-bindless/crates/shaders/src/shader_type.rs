#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Shader {
	VertexShader,
	TesselationControlShader,
	TesselationEvaluationShader,
	GeometryShader,
	FragmentShader,
	ComputeShader,
	TaskShader,
	MeshShader,
}

pub trait ShaderType {
	const SHADER: Shader;
}

pub struct VertexShader {}
impl ShaderType for VertexShader {
	const SHADER: Shader = Shader::VertexShader;
}

pub struct TesselationControlShader {}
impl ShaderType for TesselationControlShader {
	const SHADER: Shader = Shader::TesselationControlShader;
}

pub struct TesselationEvaluationShader {}
impl ShaderType for TesselationEvaluationShader {
	const SHADER: Shader = Shader::TesselationEvaluationShader;
}

pub struct GeometryShader {}
impl ShaderType for GeometryShader {
	const SHADER: Shader = Shader::GeometryShader;
}

pub struct FragmentShader {}
impl ShaderType for FragmentShader {
	const SHADER: Shader = Shader::FragmentShader;
}

pub struct ComputeShader {}
impl ShaderType for ComputeShader {
	const SHADER: Shader = Shader::ComputeShader;
}

pub struct TaskShader {}
impl ShaderType for TaskShader {
	const SHADER: Shader = Shader::TaskShader;
}

pub struct MeshShader {}
impl ShaderType for MeshShader {
	const SHADER: Shader = Shader::MeshShader;
}
