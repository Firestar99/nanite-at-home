pub trait ShaderType {}

pub struct VertexShader {}
impl ShaderType for VertexShader {}

pub struct TesselationControlShader {}
impl ShaderType for TesselationControlShader {}

pub struct TesselationEvaluationShader {}
impl ShaderType for TesselationEvaluationShader {}

pub struct GeometryShader {}
impl ShaderType for GeometryShader {}

pub struct FragmentShader {}
impl ShaderType for FragmentShader {}

pub struct ComputeShader {}
impl ShaderType for ComputeShader {}

pub struct TaskShader {}
impl ShaderType for TaskShader {}

pub struct MeshShader {}
impl ShaderType for MeshShader {}
