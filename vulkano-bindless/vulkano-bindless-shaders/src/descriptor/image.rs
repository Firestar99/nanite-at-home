use crate::descriptor::descriptor_type::{private, DescEnum, DescType};

pub use spirv_std::image::Image;
pub use spirv_std::image::SampleType;

impl<
		SampledType: SampleType<FORMAT, COMPONENTS> + Send + Sync + 'static,
		const DIM: u32,
		const DEPTH: u32,
		const ARRAYED: u32,
		const MULTISAMPLED: u32,
		const SAMPLED: u32,
		const FORMAT: u32,
		const COMPONENTS: u32,
	> private::SealedTrait for Image<SampledType, DIM, DEPTH, ARRAYED, MULTISAMPLED, SAMPLED, FORMAT, COMPONENTS>
{
}

impl<
		SampledType: SampleType<FORMAT, COMPONENTS> + Send + Sync + 'static,
		const DIM: u32,
		const DEPTH: u32,
		const ARRAYED: u32,
		const MULTISAMPLED: u32,
		const SAMPLED: u32,
		const FORMAT: u32,
		const COMPONENTS: u32,
	> DescType for Image<SampledType, DIM, DEPTH, ARRAYED, MULTISAMPLED, SAMPLED, FORMAT, COMPONENTS>
{
	type AccessType<'a> = &'a Self;
	const DESC_ENUM: DescEnum = DescEnum::Image;
}
