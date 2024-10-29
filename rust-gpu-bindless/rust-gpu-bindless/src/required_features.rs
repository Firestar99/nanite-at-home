use ash::vk::PhysicalDeviceVulkan12Features;

pub const REQUIRED_FEATURES_VK12: PhysicalDeviceVulkan12Features = PhysicalDeviceVulkan12Features::default()
	.vulkan_memory_model(true)
	.runtime_descriptor_array(true)
	.descriptor_binding_update_unused_while_pending(true)
	.descriptor_binding_partially_bound(true)
	.descriptor_binding_storage_buffer_update_after_bind(true)
	.descriptor_binding_sampled_image_update_after_bind(true)
	.descriptor_binding_storage_image_update_after_bind(true)
	.descriptor_binding_uniform_buffer_update_after_bind(true);
