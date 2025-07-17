use crate::descriptor::ImageTypeImpl;
use spirv_std::image::SampleType;
use spirv_std::image::{Image as SpvImage, ImageFormat};

pub trait SpvImageToBindlessImage {
	/// The bindless Image of a spirv-std Image, discarding some of its generics
	type Image;
}

impl<
	SampledType: SampleType<{ ImageFormat::Unknown as u32 }, 4>,
	const DIM: u32,
	const DEPTH: u32,
	const ARRAYED: u32,
	const MULTISAMPLED: u32,
	const SAMPLED: u32,
> SpvImageToBindlessImage
	for SpvImage<SampledType, DIM, DEPTH, ARRAYED, MULTISAMPLED, SAMPLED, { ImageFormat::Unknown as u32 }, 4>
{
	type Image = ImageTypeImpl<SampledType, DIM, ARRAYED, MULTISAMPLED>;
}

/// A 2d image used with a sampler. This is pretty typical and probably what you want.
pub type Image2d = <spirv_std::image::Image2d as SpvImageToBindlessImage>::Image;
/// A 3d image used with a sampler.
pub type Image3d = <spirv_std::image::Image3d as SpvImageToBindlessImage>::Image;
/// A 2d image used with a sampler, containing unsigned integer data.
pub type Image2dU = <spirv_std::image::Image2dU as SpvImageToBindlessImage>::Image;
/// A 3d image used with a sampler, containing unsigned integer data.
pub type Image3dU = <spirv_std::image::Image3dU as SpvImageToBindlessImage>::Image;
/// A 2d image used with a sampler, containing signed integer data.
pub type Image2dI = <spirv_std::image::Image2dI as SpvImageToBindlessImage>::Image;
/// A 3d image used with a sampler, containing signed integer data.
pub type Image3dI = <spirv_std::image::Image3dI as SpvImageToBindlessImage>::Image;
/// A cubemap, i.e. a cube of 6 textures, sampled using a direction rather than image coordinates.
pub type Cubemap = <spirv_std::image::Cubemap as SpvImageToBindlessImage>::Image;
