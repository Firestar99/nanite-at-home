use crate::platform::ash::{AshCreateInfo, AshExtensions};
use anyhow::anyhow;
use ash::ext::{debug_utils, mesh_shader};
use ash::vk::{
	ApplicationInfo, Bool32, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT,
	DebugUtilsMessengerCallbackDataEXT, DebugUtilsMessengerCreateInfoEXT, DeviceCreateInfo, DeviceQueueCreateInfo,
	InstanceCreateInfo, PhysicalDeviceFeatures, PhysicalDeviceType, PhysicalDeviceVulkan11Features,
	PhysicalDeviceVulkan12Features, PhysicalDeviceVulkan13Features, PipelineCacheCreateInfo, QueueFlags,
	ShaderStageFlags, ValidationFeatureEnableEXT, ValidationFeaturesEXT,
};
use ash::Entry;
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use gpu_allocator::{AllocationSizes, AllocatorDebugSettings};
use parking_lot::Mutex;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::ffi::{c_void, CStr};
use std::fmt::Debug;

pub fn required_features_vk11() -> PhysicalDeviceVulkan11Features<'static> {
	PhysicalDeviceVulkan11Features::default()
}

pub fn required_features_vk12() -> PhysicalDeviceVulkan12Features<'static> {
	PhysicalDeviceVulkan12Features::default()
		.vulkan_memory_model(true)
		.runtime_descriptor_array(true)
		.descriptor_binding_update_unused_while_pending(true)
		.descriptor_binding_partially_bound(true)
		.descriptor_binding_storage_buffer_update_after_bind(true)
		.descriptor_binding_sampled_image_update_after_bind(true)
		.descriptor_binding_storage_image_update_after_bind(true)
		.descriptor_binding_uniform_buffer_update_after_bind(true)
		.timeline_semaphore(true)
}

pub fn required_features_vk13() -> PhysicalDeviceVulkan13Features<'static> {
	PhysicalDeviceVulkan13Features::default()
		.synchronization2(true)
		.dynamic_rendering(true)
}

pub const LAYER_VALIDATION: &CStr = c"VK_LAYER_KHRONOS_validation";

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Debuggers {
	#[default]
	None,
	RenderDoc,
	Validation,
	DebugPrintf,
}

pub struct AshSingleGraphicsQueueCreateInfo<'a> {
	pub app_name: &'a CStr,
	pub app_version: u32,
	pub shader_stages: ShaderStageFlags,
	pub extensions: &'a [&'a CStr],
	pub features: PhysicalDeviceFeatures,
	pub features_vk11: PhysicalDeviceVulkan11Features<'static>,
	pub features_vk12: PhysicalDeviceVulkan12Features<'static>,
	pub features_vk13: PhysicalDeviceVulkan13Features<'static>,
	pub debug: Debuggers,
	pub debug_callback: Option<&'a DebugUtilsMessengerCreateInfoEXT<'a>>,
}

impl Default for AshSingleGraphicsQueueCreateInfo<'_> {
	fn default() -> Self {
		Self {
			app_name: c"Unknown App",
			app_version: 0,
			shader_stages: ShaderStageFlags::ALL_GRAPHICS | ShaderStageFlags::COMPUTE,
			extensions: &[],
			features: PhysicalDeviceFeatures::default(),
			features_vk11: required_features_vk11(),
			features_vk12: required_features_vk12(),
			features_vk13: required_features_vk13(),
			debug: Debuggers::default(),
			debug_callback: None,
		}
	}
}

/// Creates an [`AshCreateInfo`] with any GPU (preferring dedicated) and it's single graphics + compute queue. Can be
/// used as a simple initialization logic for small demos or testing.
///
/// If any of the steps were to fail during initialization, this method currently does not clean up after itself
/// correctly. It will only destroy itself correctly if the entire initialization succeeds.
pub fn ash_init_single_graphics_queue(
	mut create_info: AshSingleGraphicsQueueCreateInfo,
) -> anyhow::Result<AshCreateInfo> {
	unsafe {
		if matches!(create_info.debug, Debuggers::RenderDoc) {
			// renderdoc does not yet support wayland
			std::env::remove_var("WAYLAND_DISPLAY");
			std::env::set_var("ENABLE_VULKAN_RENDERDOC_CAPTURE", "1");
		}
		let entry = Entry::load()?;

		let instance = {
			let mut layers = SmallVec::<[_; 1]>::new();
			let mut extensions = SmallVec::<[_; 10]>::new();
			let mut validation_features = SmallVec::<[_; 4]>::new();

			if let Some(validation_feature_ext) = match create_info.debug {
				Debuggers::Validation => Some(ValidationFeatureEnableEXT::GPU_ASSISTED),
				Debuggers::DebugPrintf => Some(ValidationFeatureEnableEXT::DEBUG_PRINTF),
				_ => None,
			} {
				// these features may be required for anything gpu assisted to work, at least without it's complaining
				// about them missing
				create_info.features_vk12 = create_info
					.features_vk12
					.vulkan_memory_model(true)
					.vulkan_memory_model_device_scope(true);

				layers.push(LAYER_VALIDATION.as_ptr());
				validation_features.extend_from_slice(&[
					validation_feature_ext,
					ValidationFeatureEnableEXT::GPU_ASSISTED_RESERVE_BINDING_SLOT,
				]);
			}

			extensions.push(debug_utils::NAME.as_ptr());

			entry.create_instance(
				&InstanceCreateInfo::default()
					.application_info(
						&ApplicationInfo::default()
							.application_name(create_info.app_name)
							.application_version(create_info.app_version)
							.engine_name(c"rust-gpu-bindless")
							.engine_version(1)
							.api_version(ash::vk::make_api_version(0, 1, 3, 0)),
					)
					.enabled_extension_names(&extensions)
					.enabled_layer_names(&layers)
					.push_next(&mut ValidationFeaturesEXT::default().enabled_validation_features(&validation_features)),
				None,
			)?
		};

		let debug_instance = debug_utils::Instance::new(&entry, &instance);
		let debug_messager = {
			let default_callback = DebugUtilsMessengerCreateInfoEXT::default()
				.message_severity(
					DebugUtilsMessageSeverityFlagsEXT::ERROR
						| DebugUtilsMessageSeverityFlagsEXT::WARNING
						| DebugUtilsMessageSeverityFlagsEXT::INFO,
				)
				.message_type(
					DebugUtilsMessageTypeFlagsEXT::GENERAL
						| DebugUtilsMessageTypeFlagsEXT::VALIDATION
						| DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
				)
				.pfn_user_callback(Some(default_debug_callback));
			debug_instance
				.create_debug_utils_messenger(create_info.debug_callback.unwrap_or(&default_callback), None)?
		};

		let physical_device = {
			instance
				.enumerate_physical_devices()
				.unwrap()
				.into_iter()
				.min_by_key(|phy| match instance.get_physical_device_properties(*phy).device_type {
					PhysicalDeviceType::DISCRETE_GPU => 1,
					PhysicalDeviceType::VIRTUAL_GPU => 2,
					PhysicalDeviceType::INTEGRATED_GPU => 3,
					PhysicalDeviceType::CPU => 4,
					_ => 5,
				})
				.ok_or(anyhow!("No physical devices available"))?
		};

		let queue_family_index = {
			instance
				.get_physical_device_queue_family_properties(physical_device)
				.into_iter()
				.enumerate()
				.find(|(_, prop)| prop.queue_flags.contains(QueueFlags::GRAPHICS | QueueFlags::COMPUTE))
				.ok_or(anyhow!("No graphics + compute queues on physical device available"))?
				.0 as u32
		};

		let device = {
			let extensions = create_info.extensions.iter().map(|x| x.as_ptr()).collect::<Vec<_>>();
			instance.create_device(
				physical_device,
				&DeviceCreateInfo::default()
					.enabled_features(&create_info.features)
					.enabled_extension_names(&extensions)
					.push_next(&mut create_info.features_vk11)
					.push_next(&mut create_info.features_vk12)
					.push_next(&mut create_info.features_vk13)
					.queue_create_infos(&[DeviceQueueCreateInfo::default()
						.queue_family_index(queue_family_index)
						.queue_priorities(&[1.])]),
				None,
			)?
		};

		let queue = device.get_device_queue(queue_family_index, 0);
		let memory_allocator = Allocator::new(&AllocatorCreateDesc {
			instance: instance.clone(),
			device: device.clone(),
			physical_device,
			debug_settings: AllocatorDebugSettings::default(),
			buffer_device_address: false,
			allocation_sizes: AllocationSizes::default(),
		})?;
		let cache = device.create_pipeline_cache(&PipelineCacheCreateInfo::default(), None)?;

		let ext_mesh_shader = create_info
			.extensions
			.contains(&mesh_shader::NAME)
			.then(|| mesh_shader::Device::new(&instance, &device));

		let ext_debug_utils = create_info
			.extensions
			.contains(&debug_utils::NAME)
			.then(|| debug_utils::Device::new(&instance, &device));

		Ok(AshCreateInfo {
			entry,
			instance,
			physical_device,
			device,
			queue_family_index,
			queue,
			memory_allocator: Some(Mutex::new(memory_allocator)),
			shader_stages: create_info.shader_stages,
			cache: Some(cache),
			extensions: AshExtensions {
				ext_mesh_shader,
				ext_debug_utils,
				..AshExtensions::default()
			},
			destroy: Some(Box::new(move |create_info| {
				let instance = &create_info.instance;
				let device = &create_info.device;

				create_info.extensions = AshExtensions::default();
				if let Some(cache) = create_info.cache {
					device.destroy_pipeline_cache(cache, None);
				}
				drop(create_info.memory_allocator.take().unwrap());
				device.destroy_device(None);
				debug_instance.destroy_debug_utils_messenger(debug_messager, None);
				instance.destroy_instance(None);
			})),
		})
	}
}

/// All child objects created on device must have been destroyed prior to destroying device
/// https://vulkan.lunarg.com/doc/view/1.3.296.0/linux/1.3-extensions/vkspec.html#VUID-vkDestroyDevice-device-05137
const VUID_VK_DESTROY_DEVICE_DEVICE_05137: i32 = 0x4872eaa0;

const IGNORED_MSG_IDS: &[i32] = &[VUID_VK_DESTROY_DEVICE_DEVICE_05137];

unsafe extern "system" fn default_debug_callback(
	message_severity: DebugUtilsMessageSeverityFlagsEXT,
	message_type: DebugUtilsMessageTypeFlagsEXT,
	callback_data: *const DebugUtilsMessengerCallbackDataEXT<'_>,
	_p_user_data: *mut c_void,
) -> Bool32 {
	unsafe {
		let callback_data = *callback_data;
		let message_id_number = callback_data.message_id_number;
		let message_id_name = callback_data
			.message_id_name_as_c_str()
			.map_or(Cow::Borrowed(""), CStr::to_string_lossy);
		let message = callback_data
			.message_as_c_str()
			.map_or(Cow::Borrowed("No message"), CStr::to_string_lossy);
		let args =
			format!("{message_severity:?}: {message_type:?} [{message_id_name} ({message_id_number:#x})]: {message}");

		let is_error = message_severity.contains(DebugUtilsMessageSeverityFlagsEXT::ERROR);
		let is_ignored = IGNORED_MSG_IDS.contains(&message_id_number);
		if is_error {
			if is_ignored {
				eprintln!("{}", args);
			} else {
				panic!("{}", args);
			}
		} else {
			println!("{}", args);
		}

		false.into()
	}
}
