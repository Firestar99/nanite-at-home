use crate::device::init::{QueueAllocation, QueueAllocator};
use crate::device::queue_allocation_helper::{
	Priority, QueueAllocationHelper, QueueAllocationHelperEntry, QueueAllocatorHelper,
};
use std::sync::Arc;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{Queue, QueueCreateInfo, QueueFlags};
use vulkano::instance::Instance;

// queues
#[derive(Copy, Clone, Debug, Default)]
pub struct QueuesGeneric<T> {
	pub client: ClientQueuesGeneric<T>,
}

#[derive(Copy, Clone, Debug, Default)]
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
pub struct SpaceQueueAllocator {}

pub struct SpaceQueueAllocation {
	queue_ids: QueuesGeneric<QueueAllocationHelperEntry<1>>,
	allocation: QueueAllocationHelper,
}

impl SpaceQueueAllocator {
	pub fn new() -> SpaceQueueAllocator {
		Default::default()
	}
}

impl QueueAllocator<Queues, SpaceQueueAllocation> for SpaceQueueAllocator {
	fn alloc(
		self,
		_instance: &Arc<Instance>,
		_physical_device: &Arc<PhysicalDevice>,
	) -> (SpaceQueueAllocation, Vec<QueueCreateInfo>) {
		let queue_allocator = QueueAllocatorHelper::new(_physical_device);

		// graphics_main (compute and graphics) queue
		let graphics_family = queue_allocator
			.queues()
			.find(|q| q.queue_flags.contains(QueueFlags::GRAPHICS & QueueFlags::COMPUTE))
			.expect("No graphics and compute queue available!");
		let client_graphics_main = graphics_family.add(Priority(1.0));

		// async_compute queue
		let async_compute_family = queue_allocator
			.queues()
			// 1. compute but not graphics
			.find(|q| q.queue_flags.contains(QueueFlags::COMPUTE) && !q.queue_flags.contains(QueueFlags::GRAPHICS))
			// 2. compute but not selected graphics_family
			.or_else(|| {
				queue_allocator
					.queues()
					.find(|q| q.queue_flags.contains(QueueFlags::COMPUTE) && *q != graphics_family)
			})
			// 3. inherit from graphics_family if additional queues are available
			.or_else(|| (graphics_family.queue_count > 1).then_some(graphics_family));
		let client_async_compute = if let Some(async_compute) = async_compute_family {
			async_compute.add(Priority::default())
		} else {
			// 4. no dedicated compute queue: share graphics queue
			client_graphics_main
		};

		// transfer queue
		let transfer_family = queue_allocator
			.queues()
			// 1. explicit transfer but not compute or graphics
			.find(|q| {
				q.queue_flags.contains(QueueFlags::TRANSFER)
					&& !q.queue_flags.intersects(QueueFlags::GRAPHICS | QueueFlags::COMPUTE)
			})
			// 2. explicit transfer but not selected graphics_family or async_compute_family
			.or_else(|| {
				queue_allocator.queues().find(|q| {
					q.queue_flags.contains(QueueFlags::TRANSFER)
						&& *q != graphics_family
						&& async_compute_family.as_ref().map_or(true, |f| *q != *f)
				})
			})
			// 3. inherit from async_compute_family if additional queues are available
			.or_else(|| async_compute_family.and_then(|f| (f.queue_count > 1).then_some(f)))
			// 4. inherit from graphics_family if additional queues are available
			.or_else(|| (graphics_family.queue_count > 1).then_some(graphics_family));
		let client_transfer = if let Some(transfer) = transfer_family {
			// transfer queue found: create entry
			transfer.add(Priority::default())
		} else {
			// 5. no dedicated transfer queue: share compute queue (which may share graphics queue)
			client_async_compute
		};

		let (queue_ids, create_info) = queue_allocator.build();

		(
			SpaceQueueAllocation {
				queue_ids: QueuesGeneric {
					client: ClientQueuesGeneric {
						graphics_main: client_graphics_main,
						async_compute: client_async_compute,
						transfer: client_transfer,
					},
				},
				allocation: queue_ids,
			},
			create_info,
		)
	}
}

impl QueueAllocation<Queues> for SpaceQueueAllocation {
	fn take(self, queues: Vec<Arc<Queue>>) -> Queues {
		Queues {
			client: ClientQueuesGeneric {
				graphics_main: self
					.allocation
					.get_queue(queues.as_slice(), &self.queue_ids.client.graphics_main)
					.clone(),
				async_compute: self
					.allocation
					.get_queue(queues.as_slice(), &self.queue_ids.client.async_compute)
					.clone(),
				transfer: self
					.allocation
					.get_queue(queues.as_slice(), &self.queue_ids.client.transfer)
					.clone(),
			},
		}
	}
}
