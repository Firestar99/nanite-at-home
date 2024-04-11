use std::sync::Arc;
use vulkano::descriptor_set::allocator::{AllocationHandle, DescriptorSetAlloc, DescriptorSetAllocator};
use vulkano::descriptor_set::layout::{DescriptorBindingFlags, DescriptorSetLayout, DescriptorSetLayoutCreateFlags};
use vulkano::descriptor_set::pool::{
	DescriptorPool, DescriptorPoolCreateFlags, DescriptorPoolCreateInfo, DescriptorSetAllocateInfo,
};
use vulkano::device::{Device, DeviceOwned};
use vulkano::{Validated, VulkanError};

/// A [`DescriptorSetAllocator`] specialized for bindless resource tables. Its implementation is
/// very basic, only allowing you to allocate descriptor sets with a single variable count binding,
/// and always creating a new pool for each allocation. But that's perfect for bindless resource
/// tables, as old pools freed by them increasing their capacity would never be reused anyway.
pub struct BindlessDescriptorSetAllocator {
	/// only needed to impl DeviceOwned, otherwise may just not exist
	pub device: Arc<Device>,
}

impl BindlessDescriptorSetAllocator {
	pub fn new(device: Arc<Device>) -> Arc<Self> {
		Arc::new(Self { device })
	}
}

unsafe impl DeviceOwned for BindlessDescriptorSetAllocator {
	fn device(&self) -> &Arc<Device> {
		&self.device
	}
}

unsafe impl DescriptorSetAllocator for BindlessDescriptorSetAllocator {
	fn allocate(
		&self,
		layout: &Arc<DescriptorSetLayout>,
		variable_descriptor_count: u32,
	) -> Result<DescriptorSetAlloc, Validated<VulkanError>> {
		assert_eq!(
			layout.bindings().len(),
			1,
			"Descriptor set must have exactly 1 binding!"
		);
		let binding = layout.bindings().get(&0).unwrap();
		assert!(
			binding
				.binding_flags
				.contains(DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT),
			"The single binding must be a variable descriptor count binding!"
		);

		let flags = if layout
			.flags()
			.contains(DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
		{
			DescriptorPoolCreateFlags::UPDATE_AFTER_BIND
		} else {
			DescriptorPoolCreateFlags::empty()
		};
		let pool = Arc::new(DescriptorPool::new(
			layout.device().clone(),
			DescriptorPoolCreateInfo {
				max_sets: 1,
				flags,
				pool_sizes: ahash::HashMap::from_iter([(binding.descriptor_type, variable_descriptor_count)]),
				..DescriptorPoolCreateInfo::default()
			},
		)?);

		// Safety: pool and allocated descriptor set always have the same sizes and are dropped together
		let inner = unsafe {
			pool.allocate_descriptor_sets([DescriptorSetAllocateInfo {
				variable_descriptor_count,
				..DescriptorSetAllocateInfo::new(layout.clone())
			}])?
			.next()
			.unwrap()
		};

		Ok(DescriptorSetAlloc {
			inner,
			pool,
			handle: AllocationHandle::null(),
		})
	}

	unsafe fn deallocate(&self, allocation: DescriptorSetAlloc) {
		drop(allocation)
	}
}
