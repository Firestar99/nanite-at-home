use crate::platform::ash::{AshCreateInfo, AshExtensions};
use anyhow::anyhow;
use ash::Entry;
use ash::ext::{debug_utils, mesh_shader};
use ash::khr::{surface, swapchain};
use ash::vk::{
	ApplicationInfo, Bool32, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT,
	DebugUtilsMessengerCallbackDataEXT, DebugUtilsMessengerCreateInfoEXT, DeviceCreateInfo, DeviceQueueCreateInfo,
	ExtendsDeviceCreateInfo, InstanceCreateInfo, PhysicalDeviceFeatures, PhysicalDeviceType,
	PhysicalDeviceVulkan11Features, PhysicalDeviceVulkan12Features, PhysicalDeviceVulkan13Features,
	PipelineCacheCreateInfo, QueueFlags, ShaderStageFlags, ValidationFeatureEnableEXT, ValidationFeaturesEXT,
};
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use gpu_allocator::{AllocationSizes, AllocatorDebugSettings};
use parking_lot::Mutex;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::ffi::{CStr, c_void};
use std::fmt::Debug;

pub fn required_features() -> PhysicalDeviceFeatures {
	PhysicalDeviceFeatures::default()
		.shader_storage_buffer_array_dynamic_indexing(true)
		.shader_uniform_buffer_array_dynamic_indexing(true)
		.shader_storage_image_array_dynamic_indexing(true)
		.shader_sampled_image_array_dynamic_indexing(true)
}

pub fn required_features_vk11() -> PhysicalDeviceVulkan11Features<'static> {
	PhysicalDeviceVulkan11Features::default()
}

pub fn required_features_vk12() -> PhysicalDeviceVulkan12Features<'static> {
	PhysicalDeviceVulkan12Features::default()
		.runtime_descriptor_array(true)
		.descriptor_binding_update_unused_while_pending(true)
		.descriptor_binding_partially_bound(true)
		.descriptor_indexing(true)
		.descriptor_binding_storage_buffer_update_after_bind(true)
		.descriptor_binding_uniform_buffer_update_after_bind(true)
		.descriptor_binding_storage_image_update_after_bind(true)
		.descriptor_binding_sampled_image_update_after_bind(true)
		.shader_storage_buffer_array_non_uniform_indexing(true)
		.shader_uniform_buffer_array_non_uniform_indexing(true)
		.shader_storage_image_array_non_uniform_indexing(true)
		.shader_sampled_image_array_non_uniform_indexing(true)
		.timeline_semaphore(true)
		.vulkan_memory_model(true)
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
	GpuAssistedValidation,
	DebugPrintf,
}

pub struct AppConfig<'a> {
	pub name: &'a CStr,
	pub version: u32,
}

impl Default for AppConfig<'_> {
	fn default() -> Self {
		Self {
			name: c"Unknown App",
			version: 0,
		}
	}
}

pub const fn compile_time_parse(input: &'static str) -> u32 {
	match konst::primitive::parse_u32(input) {
		Ok(e) => e,
		Err(_) => unreachable!(),
	}
}

#[macro_export]
macro_rules! app_config_from_cargo {
	() => {
		$crate::platform::ash::init::AppConfig {
			name: env!("CARGO_PKG_NAME"),
			version: $crate::__private::make_api_version(
				0,
				$crate::platform::ash::init::compile_time_parse(env!("CARGO_PKG_VERSION_MAJOR")),
				$crate::platform::ash::init::compile_time_parse(env!("CARGO_PKG_VERSION_MINOR")),
				$crate::platform::ash::init::compile_time_parse(env!("CARGO_PKG_VERSION_PATCH")),
			),
		}
	};
}

pub struct AshSingleGraphicsQueueCreateInfo<'a> {
	pub app: AppConfig<'a>,
	pub shader_stages: ShaderStageFlags,
	pub instance_extensions: &'a [&'a CStr],
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
			app: Default::default(),
			shader_stages: ShaderStageFlags::ALL_GRAPHICS | ShaderStageFlags::COMPUTE,
			instance_extensions: &[],
			extensions: &[],
			features: required_features(),
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
pub fn ash_init_single_graphics_queue(create_info: AshSingleGraphicsQueueCreateInfo) -> anyhow::Result<AshCreateInfo> {
	ash_init_single_graphics_queue_with_push_next(create_info, None::<&mut PhysicalDeviceVulkan11Features>)
}

pub fn ash_init_single_graphics_queue_with_push_next(
	mut create_info: AshSingleGraphicsQueueCreateInfo,
	device_push_next: Option<&mut impl ExtendsDeviceCreateInfo>,
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
			let mut validation_features = SmallVec::<[_; 4]>::new();

			let (debug_enable, validation_feature) = match create_info.debug {
				Debuggers::Validation => (true, None),
				Debuggers::GpuAssistedValidation => (true, Some(ValidationFeatureEnableEXT::GPU_ASSISTED)),
				Debuggers::DebugPrintf => (true, Some(ValidationFeatureEnableEXT::DEBUG_PRINTF)),
				_ => (false, None),
			};
			if debug_enable {
				// these features may be required for anything gpu assisted to work, at least without it's complaining
				// about them missing
				create_info.features_vk12 = create_info
					.features_vk12
					.vulkan_memory_model(true)
					.vulkan_memory_model_device_scope(true);
				layers.push(LAYER_VALIDATION.as_ptr());

				if let Some(validation_feature) = validation_feature {
					validation_features.extend_from_slice(&[
						validation_feature,
						ValidationFeatureEnableEXT::GPU_ASSISTED_RESERVE_BINDING_SLOT,
					]);
				}
			}

			let extensions = create_info
				.instance_extensions
				.iter()
				.copied()
				.chain([debug_utils::NAME])
				.map(|c| c.as_ptr())
				.collect::<SmallVec<[_; 3]>>();

			entry.create_instance(
				&InstanceCreateInfo::default()
					.application_info(
						&ApplicationInfo::default()
							.application_name(create_info.app.name)
							.application_version(create_info.app.version)
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
			let mut device_create_info = DeviceCreateInfo::default();
			if let Some(device_push_next) = device_push_next {
				device_create_info = device_create_info.push_next(device_push_next);
			}
			instance.create_device(
				physical_device,
				&device_create_info
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

		let debug_utils = Some(debug_utils::Device::new(&instance, &device));

		let mesh_shader = create_info
			.extensions
			.contains(&mesh_shader::NAME)
			.then(|| mesh_shader::Device::new(&instance, &device));

		let surface = create_info
			.instance_extensions
			.contains(&surface::NAME)
			.then(|| surface::Instance::new(&entry, &instance));

		let swapchain = create_info
			.extensions
			.contains(&swapchain::NAME)
			.then(|| swapchain::Device::new(&instance, &device));

		Ok(AshCreateInfo {
			entry,
			instance,
			physical_device,
			device,
			queue_family_index,
			queue: Mutex::new(queue),
			memory_allocator: Some(Mutex::new(memory_allocator)),
			shader_stages: create_info.shader_stages,
			cache: Some(cache),
			extensions: AshExtensions {
				mesh_shader,
				debug_utils,
				surface,
				swapchain,
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
		let _is_ignored = IGNORED_MSG_IDS.contains(&message_id_number);
		if is_error {
			eprintln!("{}", args);
		} else {
			println!("{}", args);
		}

		false.into()
	}
}
