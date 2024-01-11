use vulkano::device::Features;

pub const REQUIRED_FEATURES: Features = Features {
	vulkan_memory_model: true,
	runtime_descriptor_array: true,
	descriptor_binding_variable_descriptor_count: true,
	..Features::empty()
};
