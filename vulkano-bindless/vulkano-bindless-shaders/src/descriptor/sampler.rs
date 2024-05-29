use crate::descriptor::descriptor_type::{private, DescEnum, DescType};

pub use spirv_std::Sampler;

impl private::SealedTrait for Sampler {}

impl DescType for Sampler {
	type AccessType<'a> = &'a Sampler;
	const DESC_ENUM: DescEnum = DescEnum::Sampler;
}
