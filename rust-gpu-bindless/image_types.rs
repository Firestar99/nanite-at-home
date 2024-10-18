// This file declares the common image formats that are included by default accessible with the Descriptors struct
// StorageImage1d's are broken, rust-gpu emits them as potentially sampleable which requires `Sampled1D` feature
// And Arrays are just unnecessary imo with bindless

macro_rules! standard_image_types {
	($macro_name:ident) => {
		$macro_name! {
			{
				// storage_image_1d: ::spirv_std::image::StorageImage1d,
				storage_image_2d: ::spirv_std::image::StorageImage2d,
				storage_image_3d: ::spirv_std::image::StorageImage3d,
				// storage_image_1du: ::spirv_std::image::StorageImage1dU,
				storage_image_2du: ::spirv_std::image::StorageImage2dU,
				storage_image_3du: ::spirv_std::image::StorageImage3dU,
				// storage_image_1di: ::spirv_std::image::StorageImage1dI,
				storage_image_3di: ::spirv_std::image::StorageImage3dI,
				storage_image_2di: ::spirv_std::image::StorageImage2dI,
			} {
				// image_1d: ::spirv_std::image::Image1d,
				image_2d: ::spirv_std::image::Image2d,
				image_3d: ::spirv_std::image::Image3d,
				// image_1du: ::spirv_std::image::Image1dU,
				image_2du: ::spirv_std::image::Image2dU,
				image_3du: ::spirv_std::image::Image3dU,
				// image_1di: ::spirv_std::image::Image1dI,
				image_2di: ::spirv_std::image::Image2dI,
				image_3di: ::spirv_std::image::Image3dI,
				// image_1d_array: ::spirv_std::image::Image1dArray,
				// image_2d_array: ::spirv_std::image::Image2dArray,
				// image_3d_array: ::spirv_std::image::Image3dArray,
				// image_1du_array: ::spirv_std::image::Image1dUArray,
				// image_2du_array: ::spirv_std::image::Image2dUArray,
				// image_3du_array: ::spirv_std::image::Image3dUArray,
				// image_1di_array: ::spirv_std::image::Image1dIArray,
				// image_2di_array: ::spirv_std::image::Image2dIArray,
				// image_3di_array: ::spirv_std::image::Image3dIArray,
				cubemap: ::spirv_std::image::Cubemap,
			}
		}
	};
}

pub(crate) use standard_image_types;
