use vulkano::device::DeviceFeatures;

pub const REQUIRED_FEATURES: DeviceFeatures = DeviceFeatures {
	vulkan_memory_model: true,
	runtime_descriptor_array: true,
	descriptor_binding_variable_descriptor_count: true,
	..DeviceFeatures::empty()
};
