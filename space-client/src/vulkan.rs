use std::sync::Arc;

use vulkano::device::{Queue, QueueCreateInfo};
use vulkano::device::physical::PhysicalDevice;
use vulkano::instance::Instance;

use space_engine::application_config::ApplicationConfig;
use space_engine::vulkan::init::{init, Init, Plugin};
use space_engine::vulkan::plugins::renderdoc_layer_plugin::RenderdocLayerPlugin;
use space_engine::vulkan::plugins::standard_validation_layer_plugin::StandardValidationLayerPlugin;
use space_engine::vulkan::queue_allocator::{QueueAllocation, QueueAllocationHelper, QueueAllocationHelperEntry, QueueAllocator};

use crate::cli_args::Cli;

pub fn create_vulkan_instance_and_device(application_config: ApplicationConfig, cli: &Cli) -> Init<Queues> {
	let mut plugins: Vec<&mut dyn Plugin> = vec![];

	let mut standard_validation_plugin = StandardValidationLayerPlugin {};
	if cli.validation_layer {
		plugins.push(&mut standard_validation_plugin);
	}
	let mut renderdoc_plugin = RenderdocLayerPlugin {};
	if cli.renderdoc {
		plugins.push(&mut renderdoc_plugin);
	}

	init(application_config, plugins, ClientQueueAllocator::new())
}


// queues
#[derive(Default)]
pub struct QueuesGeneric<T> {
	pub client: ClientQueuesGeneric<T>,
}

#[derive(Default)]
pub struct ClientQueuesGeneric<T> {
	/// graphics and compute queue
	pub graphics_main: T,
	/// async compute queue if available, or graphics_main
	pub async_compute: T,
	/// async transfer queue if available, or async_compute
	pub transfer: T,
}

pub type Queues = QueuesGeneric<Arc<Queue>>;


// queue allocator
#[derive(Default)]
struct ClientQueueAllocator {
	queue_ids: QueuesGeneric<QueueAllocationHelperEntry>,
	allocation: Option<QueueAllocation>,
}

impl ClientQueueAllocator {
	pub fn new() -> ClientQueueAllocator {
		ClientQueueAllocator { ..Default::default() }
	}
}

impl QueueAllocator<Queues> for ClientQueueAllocator {
	fn alloc<'a>(&mut self, _instance: &Arc<Instance>, _physical_device: &PhysicalDevice<'a>) -> Vec<QueueCreateInfo<'a>> {
		assert!(self.allocation.is_none());
		let mut queue_allocator = QueueAllocationHelper::new(_physical_device);

		// graphics_main (compute and graphics) queue
		let graphics_family = _physical_device.queue_families().find(|q| q.supports_graphics() && q.supports_compute())
			.expect("No graphics queue available!");
		{
			self.queue_ids.client.graphics_main = queue_allocator.add_single(graphics_family, 1.0);
		}

		// async_compute queue
		// 1. compute but not graphics
		let async_compute_family = _physical_device.queue_families().find(|q| q.supports_compute() && !q.supports_graphics())
			// 2. compute but not selected graphics_family
			.or_else(|| _physical_device.queue_families().find(|q| q.supports_compute() && *q != graphics_family))
			// 3. inherit from graphics_family if additional queues are available
			.or_else(|| (graphics_family.queues_count() > 1).then_some(graphics_family));
		if let Some(async_compute) = async_compute_family {
			self.queue_ids.client.async_compute = queue_allocator.add_single(async_compute, 0.5);
		} else {
			// 4. no dedicated compute queue: share graphics queue
			self.queue_ids.client.async_compute = self.queue_ids.client.graphics_main;
		}

		// transfer queue
		// 1. explicit transfer but not compute or graphics
		let transfer_family = _physical_device.queue_families().find(|q| q.explicitly_supports_transfers() && !q.supports_compute() && !q.supports_graphics())
			// 2. explicit transfer but not selected graphics_family or async_compute_family
			.or_else(|| _physical_device.queue_families().find(|q| q.explicitly_supports_transfers() && *q != graphics_family && async_compute_family.map_or(true, |f| *q != f)))
			// 3. inherit from async_compute_family if additional queues are available
			.or_else(|| async_compute_family.and_then(|f| (f.queues_count() > 1).then_some(f)))
			// 4. inherit from graphics_family if additional queues are available
			.or_else(|| (graphics_family.queues_count() > 1).then_some(graphics_family));
		if let Some(transfer) = transfer_family {
			// transfer queue found: create entry
			self.queue_ids.client.transfer = queue_allocator.add_single(transfer, 0.5);
		} else {
			// 5. no dedicated transfer queue: share compute queue (which may share graphics queue)
			self.queue_ids.client.transfer = self.queue_ids.client.async_compute;
		}

		let (allocation, create_info) = queue_allocator.build();
		self.allocation = Some(allocation);
		create_info
	}

	fn process(&mut self, queues: Vec<Arc<Queue>>) -> Queues {
		if let Some(allocation) = &self.allocation {
			Queues {
				client: ClientQueuesGeneric {
					graphics_main: allocation.get_queue_single(&self.queue_ids.client.graphics_main, queues.as_slice()).clone(),
					async_compute: allocation.get_queue_single(&self.queue_ids.client.async_compute, queues.as_slice()).clone(),
					transfer: allocation.get_queue_single(&self.queue_ids.client.transfer, queues.as_slice()).clone(),
				},
			}
		} else {
			unreachable!()
		}
	}
}
