use crate::descriptor::descriptor_content::DescContent;
use core::marker::PhantomData;
pub use spirv_std::image::SampleType;
use spirv_std::image::{Arrayed, Dimensionality, Image as SpvImage, ImageDepth, ImageFormat, Multisampled, Sampled};

pub struct Image<T: ImageType> {
	_phantom: PhantomData<T>,
}

impl<T: ImageType> DescContent for Image<T> {}

pub struct MutImage<T: ImageType> {
	_phantom: PhantomData<T>,
}

impl<T: ImageType> DescContent for MutImage<T> {}

pub struct ImageTypeImpl<
	SampledType: SampleType<{ ImageFormat::Unknown as u32 }, 4>,
	const DIM: u32,          // Dimensionality,
	const ARRAYED: u32,      // Arrayed,
	const MULTISAMPLED: u32, // Multisampled,
> {
	_phantom: PhantomData<SampledType>,
}

/// A type of spirv image.
///
/// # Safety
/// Should only be implemented for `spirv_std::image::Image`, to convert from a struct with many generics to a trait
/// with simpler work with associated types.
pub unsafe trait ImageType: Sized + Send + Sync + 'static {
	const DIM: u32;
	const ARRAYED: u32;
	const MULTISAMPLED: u32;
	type SampledSpvImage;
	type StorageSpvImage;

	fn dimensionality() -> Dimensionality {
		match Self::DIM {
			0 => Dimensionality::OneD,
			1 => Dimensionality::TwoD,
			2 => Dimensionality::ThreeD,
			3 => Dimensionality::Cube,
			4 => Dimensionality::Rect,
			5 => Dimensionality::Buffer,
			6 => Dimensionality::SubpassData,
			_ => unreachable!(),
		}
	}

	fn arrayed() -> Arrayed {
		match Self::ARRAYED {
			0 => Arrayed::False,
			1 => Arrayed::True,
			_ => unreachable!(),
		}
	}

	fn multisampled() -> Multisampled {
		match Self::MULTISAMPLED {
			0 => Multisampled::False,
			1 => Multisampled::True,
			_ => unreachable!(),
		}
	}
}

unsafe impl<
	SampledType: SampleType<{ ImageFormat::Unknown as u32 }, 4>,
	const DIM: u32,
	const ARRAYED: u32,
	const MULTISAMPLED: u32,
> ImageType for ImageTypeImpl<SampledType, DIM, ARRAYED, MULTISAMPLED>
{
	const DIM: u32 = DIM;
	const ARRAYED: u32 = ARRAYED;
	const MULTISAMPLED: u32 = MULTISAMPLED;

	type SampledSpvImage = SpvImage<
		SampledType,
		DIM,
		{ ImageDepth::Unknown as u32 },
		ARRAYED,
		MULTISAMPLED,
		{ Sampled::Yes as u32 },
		{ ImageFormat::Unknown as u32 },
		4,
	>;
	type StorageSpvImage = SpvImage<
		SampledType,
		DIM,
		{ ImageDepth::Unknown as u32 },
		ARRAYED,
		MULTISAMPLED,
		{ Sampled::No as u32 },
		{ ImageFormat::Unknown as u32 },
		4,
	>;
}
