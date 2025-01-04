use crate::renderer::Init;
use space_asset_shader::meshlet::instance::MeshletInstance;
use std::collections::BTreeMap;
use std::mem::size_of;
use std::sync::Arc;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::DrawMeshTasksIndirectCommand;
use vulkano::descriptor_set::layout::{
	DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType,
};
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::shader::ShaderStages;
use vulkano::DeviceSize;

pub struct MeshletAllocationBuffer {
	pub buffer: Subbuffer<[u32]>,
	pub indirect_draw_args: Subbuffer<DrawMeshTasksIndirectCommand>,
	pub descriptor_set: Arc<DescriptorSet>,
}

impl MeshletAllocationBuffer {
	pub fn new(init: &Arc<Init>, meshlet_instance_capacity: usize) -> Self {
		let buffer = Buffer::new_slice(
			init.memory_allocator.clone(),
			BufferCreateInfo {
				usage: BufferUsage::STORAGE_BUFFER,
				..BufferCreateInfo::default()
			},
			AllocationCreateInfo {
				memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
				..AllocationCreateInfo::default()
			},
			(meshlet_instance_capacity * size_of::<MeshletInstance>()) as DeviceSize,
		)
		.unwrap();
		let indirect_draw_args = Buffer::new_sized(
			init.memory_allocator.clone(),
			BufferCreateInfo {
				usage: BufferUsage::STORAGE_BUFFER | BufferUsage::INDIRECT_BUFFER,
				..BufferCreateInfo::default()
			},
			AllocationCreateInfo {
				memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
				..AllocationCreateInfo::default()
			},
		)
		.unwrap();

		let descriptor_set_layout = DescriptorSetLayout::new(
			init.device.clone(),
			DescriptorSetLayoutCreateInfo {
				bindings: BTreeMap::from_iter((0..2).map(|i| {
					(
						i as u32,
						DescriptorSetLayoutBinding {
							stages: ShaderStages::COMPUTE | ShaderStages::MESH,
							..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::StorageBuffer)
						},
					)
				})),
				..DescriptorSetLayoutCreateInfo::default()
			},
		)
		.unwrap();
		let descriptor_set = DescriptorSet::new(
			init.descriptor_allocator.clone(),
			descriptor_set_layout.clone(),
			[
				WriteDescriptorSet::buffer(0, buffer.clone()),
				WriteDescriptorSet::buffer(1, indirect_draw_args.clone()),
			],
			[],
		)
		.unwrap();
		Self {
			buffer,
			indirect_draw_args,
			descriptor_set,
		}
	}

	pub fn reset(&self) {
		*self.indirect_draw_args.write().unwrap() = DrawMeshTasksIndirectCommand {
			group_count_x: 0,
			group_count_y: 1,
			group_count_z: 1,
		};
	}
}
