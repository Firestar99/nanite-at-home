use std::sync::Arc;

use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Features};
use vulkano::device::physical::PhysicalDevice;
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::Version;

use crate::application_config::ApplicationConfig;
use crate::vulkan::ENGINE_APPLICATION_CONFIG;
use crate::vulkan::debug::Debug;
use crate::vulkan::platform::VkPlatform;
use crate::vulkan::queue_allocator::QueueAllocator;

pub trait Plugin {
	/// Return what InstanceExtensions or validation layer names you would like to be enabled.
	/// Note that you must check that said InstanceExtensions or validation layers are available,
	/// requesting something that the PhysicalDevice does not support will panic!
	fn instance_config(&mut self, _platform: &VkPlatform) -> (InstanceExtensions, Vec<&'static str>) {
		(InstanceExtensions::none(), Vec::new())
	}

	/// Check a PhysicalDevice and either disallow it or give it a score to be selected.
	/// All scores are accumulated and the highest PhysicalDevice allowed by everyone wins.
	///
	/// # Returns
	/// * to disallow a device return `None`
	/// * to set a priority for a Device return `Some(priority)`
	/// * allow the device without any priority changes return `Some(0)`
	fn physical_device_filter(&mut self, _instance: &Arc<Instance>, _physical_device: &PhysicalDevice) -> Option<i32> {
		Some(0)
	}

	/// Return what DeviceExtensions and Features you would like to be enabled.
	/// Note that you must check that said DeviceExtensions or Features are available on the
	/// PhysicalDevice, requesting something that the PhysicalDevice does not support will panic!
	fn device_config(&mut self, _instance: &Arc<Instance>, _physical_device: &PhysicalDevice) -> (DeviceExtensions, Features) {
		(DeviceExtensions::none(), Features::none())
	}
}

pub struct Init<Q> {
	pub instance: Arc<Instance>,
	pub device: Arc<Device>,
	pub queues: Q,
	_debug: Debug,
}

pub fn init<Q, A>(application_config: ApplicationConfig, mut plugins: Vec<&mut dyn Plugin>, mut queue_allocator: A) -> Init<Q>
	where
		A: QueueAllocator<Q>,
{
	let platform = VkPlatform::new();

	// instance
	let configs: Vec<_> = plugins.iter_mut()
		.map(|p| p.instance_config(&platform))
		.collect();
	let mut extensions = configs.iter()
		.map(|e| e.0)
		.reduce(|a, b| a.union(&b))
		.unwrap_or(InstanceExtensions::none());
	let layers: Vec<_> = configs.into_iter()
		.flat_map(|e| e.1)
		.map(|s| String::from(s))
		.collect();

	// debug
	extensions.ext_debug_utils = true;

	// instance
	let instance = Instance::new(InstanceCreateInfo {
		engine_name: Some(String::from(ENGINE_APPLICATION_CONFIG.name)),
		engine_version: Version::from(ENGINE_APPLICATION_CONFIG.version),
		application_name: Some(String::from(application_config.name)),
		application_version: Version::from(application_config.version),
		enabled_extensions: extensions,
		enabled_layers: layers,
		..Default::default()
	}).unwrap();

	// debug
	let _debug = Debug::new(&instance);

	// physical device selection
	let physical_device = PhysicalDevice::enumerate(&instance)
		.filter_map(|phy| {
			plugins.iter_mut()
				.map(|p| p.physical_device_filter(&instance, &phy))
				.reduce(|oa, ob| oa.map_or(None, |a| ob.map_or(None, |b| Some(a + b))))
				.unwrap_or(Some(0))
				.map_or(None, |p| Some((phy, p)))
		})
		.min_by_key(|(_, priority)| *priority)
		.expect("No suitable PhysicalDevice was found!")
		.0;

	// device extensions and features
	let (device_extensions, device_features) = plugins.iter_mut()
		.map(|p| p.device_config(&instance, &physical_device))
		.reduce(|a, b| (a.0.union(&b.0), device_features_union(&a.1, &b.1)))
		.unwrap_or((DeviceExtensions::none(), Features::none()));

	// device
	let queue_create_infos = queue_allocator.alloc(&instance, &physical_device);
	let (device, queues) = Device::new(physical_device, DeviceCreateInfo {
		enabled_extensions: device_extensions,
		enabled_features: device_features,
		queue_create_infos,
		..Default::default()
	}).unwrap();

	let queues = queue_allocator.process(queues.collect());

	Init {
		instance,
		device,
		queues,
		_debug,
	}
}

fn device_features_union(a: &Features, b: &Features) -> Features {
	Features::all().difference(a).difference(b)
}
