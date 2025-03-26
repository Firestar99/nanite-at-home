// This file declares the common image formats that are included by default accessible with the Descriptors struct
// StorageImage1d's are broken, rust-gpu emits them as potentially sampleable which requires `Sampled1D` feature
// And Arrays are just unnecessary imo with bindless

macro_rules! standard_image_types {
	($macro_name:ident) => {
		$macro_name! {
			Image2d: image_2d storage_image_2d,
			Image3d: image_3d storage_image_3d,
			Image2dU: image_2du storage_image_2du,
			Image3dU: image_3du storage_image_3du,
			Image2dI: image_2di storage_image_2di,
			Image3dI: image_3di storage_image_3di,
			Cubemap: cubemap storage_cubemap,
		}
	};
}

pub(crate) use standard_image_types;
