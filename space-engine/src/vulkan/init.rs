use std::sync::Arc;

use smallvec::SmallVec;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::descriptor_set::allocator::{StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Features, Queue, QueueCreateInfo};
use vulkano::instance::{Instance, InstanceCreateFlags, InstanceCreateInfo, InstanceExtensions, InstanceOwned};
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::{Version, VulkanLibrary};

use crate::application_config::ApplicationConfig;
use crate::vulkan::debug::Debug;
use crate::vulkan::validation_layers::ValidationLayers;
use crate::vulkan::ENGINE_APPLICATION_CONFIG;

pub trait Plugin {
	/// Return what InstanceExtensions or validation layer names you would like to be enabled.
	/// Note that you must check that said InstanceExtensions or validation layers are available,
	/// requesting something that the PhysicalDevice does not support will panic!
	fn instance_config(
		&self,
		_library: &Arc<VulkanLibrary>,
		_layers: &ValidationLayers,
	) -> (InstanceExtensions, SmallVec<[String; 1]>) {
		(InstanceExtensions::empty(), SmallVec::new())
	}

	/// Return what DeviceExtensions and Features you would like to be enabled.
	/// Note that you must check that said DeviceExtensions or Features are available on the
	/// PhysicalDevice, requesting something that the PhysicalDevice does not support will panic!
	fn device_config(&self, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, Features) {
		(DeviceExtensions::empty(), Features::empty())
	}
}

pub trait QueueAllocator<Q: 'static, ALLOCATION: QueueAllocation<Q>> {
	fn alloc(
		self,
		_instance: &Arc<Instance>,
		_physical_device: &Arc<PhysicalDevice>,
	) -> (ALLOCATION, Vec<QueueCreateInfo>);
}

pub trait QueueAllocation<Q: 'static> {
	fn take(self, queues: Vec<Arc<Queue>>) -> Q;
}

pub struct Init<Q> {
	pub device: Arc<Device>,
	pub queues: Q,
	pub memory_allocator: Arc<StandardMemoryAllocator>,
	pub descriptor_allocator: StandardDescriptorSetAllocator,
	pub cmd_buffer_allocator: StandardCommandBufferAllocator,
	_debug: Debug,
}

impl<Q: Clone> Init<Q> {
	pub fn new<ALLOCATOR, ALLOCATION>(
		application_config: ApplicationConfig,
		plugins: &[&dyn Plugin],
		queue_allocator: ALLOCATOR,
	) -> Arc<Self>
	where
		Q: 'static,
		ALLOCATOR: QueueAllocator<Q, ALLOCATION>,
		ALLOCATION: QueueAllocation<Q>,
	{
		let library = VulkanLibrary::new().unwrap();

		// instance
		let extensions;
		let layers;
		{
			let validation_layers = ValidationLayers::new(&library);
			let result = plugins
				.iter()
				.map(|p| p.instance_config(&library, &validation_layers))
				.fold((InstanceExtensions::default(), Vec::new()), |mut a, b| {
					a.1.extend(b.1);
					(a.0 | b.0, a.1)
				});
			extensions = result.0
				| InstanceExtensions {
					ext_debug_utils: true,
					..InstanceExtensions::default()
				};
			layers = result.1;
		}

		// instance
		let instance = Instance::new(
			library,
			InstanceCreateInfo {
				flags: InstanceCreateFlags::empty(),
				engine_name: Some(String::from(ENGINE_APPLICATION_CONFIG.name)),
				engine_version: Version::from(ENGINE_APPLICATION_CONFIG.version),
				application_name: Some(String::from(application_config.name)),
				application_version: Version::from(application_config.version),
				enabled_extensions: extensions,
				enabled_layers: layers,
				..Default::default()
			},
		)
		.unwrap();

		// debug
		let _debug = Debug::new(&instance);

		// physical device selection
		let physical_device = instance
			.enumerate_physical_devices()
			.unwrap()
			.max_by_key(|phy| match phy.properties().device_type {
				PhysicalDeviceType::DiscreteGpu => 4,
				PhysicalDeviceType::IntegratedGpu => 3,
				PhysicalDeviceType::VirtualGpu => 2,
				PhysicalDeviceType::Cpu => 1,
				PhysicalDeviceType::Other => 0,
				_ => -1,
			})
			.expect("No PhysicalDevice found!");

		println!(
			"Selecting physical device `{:?}`",
			physical_device.properties().device_name
		);

		// device extensions and features
		let (device_extensions, device_features) = plugins
			.iter()
			.map(|p| p.device_config(&physical_device))
			.fold((DeviceExtensions::empty(), Features::empty()), |a, b| {
				(a.0 | b.0, a.1 | b.1)
			});

		// device
		let (allocation, queue_create_infos) = queue_allocator.alloc(&instance, &physical_device);
		let (device, queues) = Device::new(
			physical_device,
			DeviceCreateInfo {
				enabled_extensions: device_extensions,
				enabled_features: device_features,
				queue_create_infos,
				..Default::default()
			},
		)
		.unwrap();
		let queues = allocation.take(queues.collect());

		let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
		let descriptor_allocator =
			StandardDescriptorSetAllocator::new(device.clone(), StandardDescriptorSetAllocatorCreateInfo::default());
		let cmd_buffer_allocator = StandardCommandBufferAllocator::new(device.clone(), Default::default());

		Arc::new(Self {
			device,
			queues,
			memory_allocator,
			descriptor_allocator,
			cmd_buffer_allocator,
			_debug,
		})
	}

	pub fn instance(&self) -> &Arc<Instance> {
		self.device.instance()
	}

	pub fn library(&self) -> &Arc<VulkanLibrary> {
		self.device.instance().library()
	}
}
