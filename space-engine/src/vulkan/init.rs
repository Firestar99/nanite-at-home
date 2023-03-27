use std::sync::Arc;

use vulkano::{Version, VulkanLibrary};
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Features, Queue, QueueCreateInfo};
use vulkano::device::physical::PhysicalDevice;
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};

use crate::application_config::ApplicationConfig;
use crate::vulkan::debug::Debug;
use crate::vulkan::ENGINE_APPLICATION_CONFIG;
use crate::vulkan::init::DevicePriority::{Allow, Disallow};
use crate::vulkan::platform::VulkanLayers;

pub enum DevicePriority {
	Disallow,
	Allow(i32),
}

pub trait Plugin {
	/// Return what InstanceExtensions or validation layer names you would like to be enabled.
	/// Note that you must check that said InstanceExtensions or validation layers are available,
	/// requesting something that the PhysicalDevice does not support will panic!
	fn instance_config(&mut self, _library: &Arc<VulkanLibrary>, _layers: &VulkanLayers) -> (InstanceExtensions, Vec<&'static str>) {
		(InstanceExtensions::empty(), Vec::new())
	}

	/// Check a PhysicalDevice and either disallow it or give it a score to be selected.
	/// All scores are accumulated and the highest PhysicalDevice allowed by everyone wins.
	///
	/// # Returns
	/// * to disallow a device return `Disallow`
	/// * to set a priority for a Device return `Allow(priority)`
	/// * allow the device without any priority changes return `Allow(0)`
	fn physical_device_filter(&mut self, _library: &Arc<VulkanLibrary>, _instance: &Arc<Instance>, _physical_device: &PhysicalDevice) -> DevicePriority {
		Allow(0)
	}

	/// Return what DeviceExtensions and Features you would like to be enabled.
	/// Note that you must check that said DeviceExtensions or Features are available on the
	/// PhysicalDevice, requesting something that the PhysicalDevice does not support will panic!
	fn device_config(&mut self, _library: &Arc<VulkanLibrary>, _instance: &Arc<Instance>, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, Features) {
		(DeviceExtensions::empty(), Features::empty())
	}
}

pub trait QueueAllocator<Q: 'static, ALLOCATION: QueueAllocation<Q>> {
	fn alloc(self, _instance: &Arc<Instance>, _physical_device: &Arc<PhysicalDevice>) -> (ALLOCATION, Vec<QueueCreateInfo>);
}

pub trait QueueAllocation<Q: 'static> {
	fn take(self, queues: Vec<Arc<Queue>>) -> Q;
}

pub struct Init<Q> {
	pub library: Arc<VulkanLibrary>,
	pub instance: Arc<Instance>,
	pub device: Arc<Device>,
	pub queues: Q,
	_debug: Debug,
}

pub fn init<Q, ALLOCATOR, ALLOCATION>(application_config: ApplicationConfig, mut plugins: Vec<&mut dyn Plugin>, queue_allocator: ALLOCATOR) -> Init<Q>
	where
		Q: 'static,
		ALLOCATOR: QueueAllocator<Q, ALLOCATION>,
		ALLOCATION: QueueAllocation<Q>
{
	let library = VulkanLibrary::new().unwrap();
	let platform = VulkanLayers::new(&library);

	// instance
	let configs: Vec<_> = plugins.iter_mut()
		.map(|p| p.instance_config(&library, &platform))
		.collect();
	let mut extensions = configs.iter()
		.map(|e| e.0)
		.reduce(|a, b| a.union(&b))
		.unwrap_or(InstanceExtensions::empty());
	let layers: Vec<_> = configs.into_iter()
		.flat_map(|e| e.1)
		.map(String::from)
		.collect();

	// debug
	extensions.ext_debug_utils = true;

	// instance
	let instance = Instance::new(library.clone(), InstanceCreateInfo {
		engine_name: Some(String::from(ENGINE_APPLICATION_CONFIG.name)),
		engine_version: Version::from(ENGINE_APPLICATION_CONFIG.version),
		application_name: Some(String::from(application_config.name)),
		application_version: Version::from(application_config.version),
		enabled_extensions: extensions,
		enabled_layers: layers,
		// allow MoltenVK
		enumerate_portability: true,
		..Default::default()
	}).unwrap();

	// debug
	let debug = Debug::new(&instance);

	// physical device selection
	let physical_device = instance.enumerate_physical_devices().unwrap()
		.filter_map(|phy| {
			let priority = plugins.iter_mut()
				.map(|p| p.physical_device_filter(&library, &instance, &phy))
				.reduce(|a, b| match a {
					Disallow => { Disallow }
					Allow(ap) => {
						match b {
							Disallow => { Disallow }
							Allow(bp) => { Allow(ap + bp) }
						}
					}
				})
				.unwrap_or(Allow(0));
			match priority {
				Disallow => { None }
				Allow(p) => { Some((phy, p)) }
			}
		})
		.min_by_key(|(_, priority)| *priority)
		.expect("No suitable PhysicalDevice was found!")
		.0;

	// device extensions and features
	let (device_extensions, device_features) = plugins.iter_mut()
		.map(|p| p.device_config(&library, &instance, &physical_device))
		.reduce(|a, b| (DeviceExtensions::union(&a.0, &b.0), Features::union(&a.1, &b.1)))
		.unwrap_or((DeviceExtensions::empty(), Features::empty()));

	// device
	let (allocation, queue_create_infos) = queue_allocator.alloc(&instance, &physical_device);
	let (device, queues) = Device::new(physical_device, DeviceCreateInfo {
		enabled_extensions: device_extensions,
		enabled_features: device_features,
		queue_create_infos,
		..Default::default()
	}).unwrap();
	let queues = allocation.take(queues.collect());

	Init {
		library,
		instance,
		device,
		queues,
		_debug: debug,
	}
}
