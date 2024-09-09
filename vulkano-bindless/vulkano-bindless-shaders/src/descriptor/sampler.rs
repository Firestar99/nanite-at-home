use crate::descriptor::descriptor_content::{private, DescContent, DescContentType};

pub use spirv_std::Sampler;

impl private::SealedTrait for Sampler {}

impl DescContent for Sampler {
	type AccessType<'a> = &'a Sampler;
	const CONTENT_TYPE: DescContentType = DescContentType::Sampler;
}
