use crate::descriptor::descriptor_type_cpu::ResourceTableCpu;
use std::sync::Arc;
use vulkano::device::physical::PhysicalDevice;
use vulkano_bindless_shaders::descriptor::{BufferTable, ImageTable, SamplerTable};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct DescriptorCounts {
	pub buffers: u32,
	pub image: u32,
	pub samplers: u32,
}

impl DescriptorCounts {
	pub fn limits(phy: &Arc<PhysicalDevice>) -> Self {
		Self {
			buffers: BufferTable::max_update_after_bind_descriptors(phy),
			image: ImageTable::max_update_after_bind_descriptors(phy),
			samplers: SamplerTable::max_update_after_bind_descriptors(phy),
		}
	}

	const REASONABLE_DEFAULTS: DescriptorCounts = DescriptorCounts {
		buffers: 10_000,
		image: 10_000,
		samplers: 400,
	};

	pub fn reasonable_defaults(phy: &Arc<PhysicalDevice>) -> Self {
		Self::REASONABLE_DEFAULTS.min(Self::limits(phy))
	}

	pub fn is_within_limit(self, limit: Self) -> bool {
		// just to make sure this is updated as well
		let DescriptorCounts {
			buffers,
			image,
			samplers,
		} = self;
		buffers <= limit.buffers && image <= limit.image && samplers <= limit.samplers
	}

	pub fn min(self, other: Self) -> Self {
		Self {
			buffers: self.buffers.min(other.buffers),
			image: self.image.min(other.image),
			samplers: self.samplers.min(other.samplers),
		}
	}
}
