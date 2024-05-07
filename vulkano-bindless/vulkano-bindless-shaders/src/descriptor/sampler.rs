use crate::descriptor::descriptor_type::{private, DescType};
use crate::descriptor::descriptors::Descriptors;

pub type Sampler = spirv_std::Sampler;

impl private::SealedTrait for Sampler {}

impl DescType for Sampler {
	type AccessType<'a> = &'a Sampler;

	#[inline]
	fn access<'a>(descriptors: &'a Descriptors<'a>, id: u32) -> Self::AccessType<'a> {
		unsafe { descriptors.samplers.index(id as usize) }
	}
}
