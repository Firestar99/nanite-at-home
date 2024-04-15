use crate::descriptor::bindless_descriptor_allocator::BindlessDescriptorSetAllocator;
use crate::descriptor::buffer_table::BufferResourceTable;
use std::marker::PhantomData;
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::shader::ShaderStages;

pub struct Descriptors {
	pub device: Arc<Device>,
	pub buffer: BufferResourceTable,
	_private: PhantomData<()>,
}

pub struct DescriptorCounts {
	pub buffer_descriptors: u32,
}

impl Default for DescriptorCounts {
	fn default() -> Self {
		Self {
			buffer_descriptors: 10_000,
		}
	}
}

impl Descriptors {
	/// Creates a new Descriptors instance with which to allocate descriptors.
	///
	/// # Safety
	/// There must only be one global Descriptors instance for each [`Device`].
	pub unsafe fn new(device: Arc<Device>, stages: ShaderStages, counts: DescriptorCounts) -> Self {
		let allocator = BindlessDescriptorSetAllocator::new(device.clone());
		Self {
			buffer: BufferResourceTable::new(device.clone(), stages, allocator, counts.buffer_descriptors),
			device,
			_private: PhantomData {},
		}
	}

	pub fn flush() {}
}
