use std::sync::Arc;

use vulkano::device::{Queue, QueueCreateInfo};
use vulkano::device::physical::{PhysicalDevice, QueueFamily};
use vulkano::instance::Instance;

pub trait QueueAllocator<Q> {
	fn alloc<'a>(&mut self, _instance: &Arc<Instance>, _physical_device: &PhysicalDevice<'a>) -> Vec<QueueCreateInfo<'a>>;

	fn process(&mut self, queues: Vec<Arc<Queue>>) -> Q;
}

#[derive(Debug)]
pub struct QueueAllocationHelper<'a, 'b> {
	physical_device: &'b PhysicalDevice<'a>,
	create_infos: Vec<QueueCreateInfo<'a>>,
}

impl<'a, 'b> QueueAllocationHelper<'a, 'b> {
	pub fn new(physical_device: &'b PhysicalDevice<'a>) -> QueueAllocationHelper<'a, 'b> {
		let mut helper = QueueAllocationHelper {
			physical_device,
			create_infos: Vec::new(),
		};
		let len = physical_device.queue_families().len();
		helper.create_infos.reserve(len);
		helper.create_infos.extend(physical_device.queue_families().map(|q| QueueCreateInfo {
			family: q,
			queues: Vec::new(),
			_ne: Default::default(),
		}));
		helper
	}

	pub fn add_single(&mut self, queue_family: QueueFamily<'a>, priority: f32) -> QueueAllocationHelperEntry {
		self.add(queue_family, &[priority])
	}

	pub fn add(&mut self, queue_family: QueueFamily<'a>, priorities: &[f32]) -> QueueAllocationHelperEntry {
		assert_eq!(queue_family.physical_device().index(), self.physical_device.index(), "QueueFamily is from another PhysicalDevice!");
		let vec = &mut self.create_infos.get_mut(queue_family.id() as usize).unwrap().queues;
		let ret = QueueAllocationHelperEntry {
			queue_id: queue_family.id(),
			priority_id: vec.len() as u32,
			count: priorities.len() as u32,
		};
		vec.extend_from_slice(priorities);
		ret
	}

	pub fn build(self) -> (QueueAllocation, Vec<QueueCreateInfo<'a>>) {
		let mut index = 0;
		let mut build_infos = Vec::with_capacity(self.create_infos.len());
		let create_infos = self.create_infos.into_iter()
			.filter(|q| {
				build_infos.push(index);
				let queue_count = q.queues.len() as u32;
				index += queue_count;
				queue_count > 0
			})
			.collect();
		(QueueAllocation { build_infos }, create_infos)
	}
}

#[derive(Copy, Clone, Debug)]
pub struct QueueAllocationHelperEntry {
	queue_id: u32,
	priority_id: u32,
	count: u32,
}

impl QueueAllocationHelperEntry {
	pub fn check(&self) {
		assert!(self.queue_id != !0 && self.priority_id != !0 && self.count != !0);
	}
}

impl Default for QueueAllocationHelperEntry {
	fn default() -> Self {
		QueueAllocationHelperEntry {
			queue_id: !0,
			priority_id: !0,
			count: !0,
		}
	}
}

pub struct QueueAllocation {
	build_infos: Vec<u32>,
}

impl QueueAllocation {
	pub fn get_index(&self, entry: &QueueAllocationHelperEntry) -> usize {
		entry.check();
		(self.build_infos[entry.queue_id as usize] + entry.priority_id) as usize
	}

	pub fn get_size(&self, entry: &QueueAllocationHelperEntry) -> usize {
		entry.check();
		entry.count as usize
	}

	pub fn get_queues<'a>(&self, entry: &QueueAllocationHelperEntry, queues: &'a [Arc<Queue>]) -> &'a [Arc<Queue>] {
		entry.check();
		let start = self.get_index(entry);
		&queues[start..(start + self.get_size(entry))]
	}

	pub fn get_queue_single<'a>(&self, entry: &QueueAllocationHelperEntry, queues: &'a [Arc<Queue>]) -> &'a Arc<Queue> {
		assert_eq!(entry.count, 1);
		&self.get_queues(entry, queues)[0]
	}
}
