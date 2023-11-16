use std::cell::RefCell;
use std::ops::Deref;
use std::sync::Arc;

use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{Queue, QueueCreateInfo, QueueFamilyProperties};

#[derive(Debug)]
pub struct QueueAllocatorHelper {
	physical_device: Arc<PhysicalDevice>,
	priorities: Vec<RefCell<Vec<f32>>>,
}

#[derive(Copy, Clone, Debug)]
pub struct QueueFamilyInfo<'a> {
	parent: &'a QueueAllocatorHelper,
	id: u32,
	properties: &'a QueueFamilyProperties,
}

impl QueueAllocatorHelper {
	pub fn new(physical_device: &Arc<PhysicalDevice>) -> QueueAllocatorHelper {
		QueueAllocatorHelper {
			physical_device: physical_device.clone(),
			priorities: vec![RefCell::new(Vec::new()); physical_device.queue_family_properties().len()],
		}
	}

	pub fn queues(&self) -> impl Iterator<Item = QueueFamilyInfo> {
		self.physical_device
			.queue_family_properties()
			.iter()
			.enumerate()
			.map(|q| QueueFamilyInfo {
				parent: self,
				id: q.0 as u32,
				properties: q.1,
			})
	}
}

impl<'a> PartialEq<Self> for QueueFamilyInfo<'a> {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}

impl<'a> Eq for QueueFamilyInfo<'a> {}

#[derive(Copy, Clone, Debug)]
pub struct Priority(pub f32);

impl Default for Priority {
	/// default vulkan queue priority is 0.5
	fn default() -> Self {
		Self(0.5)
	}
}

impl From<f32> for Priority {
	fn from(value: f32) -> Self {
		Priority(value)
	}
}

impl<'a> QueueFamilyInfo<'a> {
	pub fn add(&self, priority: Priority) -> QueueAllocationHelperEntry<1> {
		self.add_multiple([priority; 1])
	}

	pub fn add_multiple<const N: usize>(&self, priorities: [Priority; N]) -> QueueAllocationHelperEntry<N> {
		let mut p = self.parent.priorities.get(self.id as usize).unwrap().borrow_mut();
		let index = p.len() as u32;
		p.extend_from_slice(&priorities.map(|p| p.0));
		QueueAllocationHelperEntry {
			queue_id: self.id,
			priority_index: index,
		}
	}
}

impl<'a> Deref for QueueFamilyInfo<'a> {
	type Target = QueueFamilyProperties;

	fn deref(&self) -> &Self::Target {
		self.properties
	}
}

impl QueueAllocatorHelper {
	pub fn build(self) -> (QueueAllocationHelper, Vec<QueueCreateInfo>) {
		let mut index: u32 = 0;
		let mut build_infos = Vec::with_capacity(self.priorities.len());
		let create_infos = self
			.priorities
			.into_iter()
			.enumerate()
			.filter_map(|mut q| {
				build_infos.push(index);
				let queue_count = q.1.get_mut().len() as u32;
				index += queue_count;
				(queue_count > 0).then_some(q)
			})
			.map(|q| QueueCreateInfo {
				queue_family_index: q.0 as u32,
				queues: q.1.into_inner(),
				..Default::default()
			})
			.collect();
		(
			QueueAllocationHelper {
				physical_device: self.physical_device,
				build_infos,
			},
			create_infos,
		)
	}
}

#[derive(Copy, Clone, Debug)]
pub struct QueueAllocationHelperEntry<const N: usize> {
	queue_id: u32,
	priority_index: u32,
}

#[derive(Debug)]
pub struct QueueAllocationHelper {
	physical_device: Arc<PhysicalDevice>,
	build_infos: Vec<u32>,
}

impl QueueAllocationHelper {
	fn get_start_index<const N: usize>(&self, entry: &QueueAllocationHelperEntry<N>) -> u32 {
		self.build_infos[entry.queue_id as usize] + entry.priority_index
	}

	pub fn get_queue<'a>(&self, queues: &'a [Arc<Queue>], entry: &QueueAllocationHelperEntry<1>) -> &'a Arc<Queue> {
		assert_eq!(
			**queues.first().unwrap().device().physical_device(),
			*self.physical_device,
			"A different physical device between QueueAllocatorHelper and submitted queueus!"
		);
		let start = self.get_start_index(entry) as usize;
		&queues[start]
	}

	pub fn get_queues<'a, const N: usize>(
		&self,
		queues: &'a [Arc<Queue>],
		entry: &QueueAllocationHelperEntry<N>,
	) -> &'a [Arc<Queue>; N] {
		assert_eq!(
			**queues.first().unwrap().device().physical_device(),
			*self.physical_device,
			"A different physical device between QueueAllocatorHelper and submitted queueus!"
		);
		let start = self.get_start_index(entry) as usize;
		queues[start..(start + N)]
			.try_into()
			.expect("Allocated queues don't match QueueAllocatorHelper state!")
	}
}
