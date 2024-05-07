use crate::descriptor::descriptor_type::{private, DescType};
use crate::descriptor::descriptors::Descriptors;

pub use spirv_std::image::Image;
pub use spirv_std::image::SampleType;

pub type SampledImage2D = Image!(2D, type=f32, sampled);

impl<
		SampledType: SampleType<FORMAT, COMPONENTS>,
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

impl DescType for SampledImage2D {
	type AccessType<'a> = &'a Self;

	#[inline]
	fn access<'a>(descriptors: &'a Descriptors<'a>, id: u32) -> Self::AccessType<'a> {
		unsafe { descriptors.sampled_images_2d.index(id as usize) }
	}
}
