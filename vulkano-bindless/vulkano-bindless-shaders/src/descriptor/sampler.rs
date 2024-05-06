use crate::descriptor::descriptor_type::{private, DescType, ResourceTable};
use crate::descriptor::descriptors::Descriptors;

pub type Sampler = spirv_std::Sampler;

pub struct SamplerTable;

impl private::SealedTrait for Sampler {}

impl private::SealedTrait for SamplerTable {}

impl DescType for Sampler {
	type ResourceTable = SamplerTable;
	type AccessType<'a> = &'a Sampler;

	#[inline]
	fn access<'a>(descriptors: &'a Descriptors<'a>, id: u32) -> Self::AccessType<'a> {
		unsafe { descriptors.samplers.index(id as usize) }
	}
}

impl ResourceTable for SamplerTable {}
