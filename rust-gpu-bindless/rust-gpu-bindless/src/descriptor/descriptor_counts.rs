use crate::descriptor::BindlessCreateInfo;
use crate::platform::interface::BindlessPlatform;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct DescriptorCounts {
	pub buffers: u32,
	pub image: u32,
	pub samplers: u32,
}

impl DescriptorCounts {
	pub fn limits<P: BindlessPlatform>(ci: &BindlessCreateInfo<P>) -> Self {
		unsafe { P::update_after_bind_descriptor_limits(ci) }
	}

	const REASONABLE_DEFAULTS: Self = DescriptorCounts {
		buffers: 10_000,
		image: 10_000,
		samplers: 400,
	};

	pub fn reasonable_defaults<P: BindlessPlatform>(ci: &BindlessCreateInfo<P>) -> Self {
		Self::REASONABLE_DEFAULTS.min(Self::limits(ci))
	}

	pub fn is_within_limit(&self, limit: Self) -> bool {
		// just to make sure this is updated as well
		let DescriptorCounts {
			buffers,
			image,
			samplers,
		} = *self;
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
