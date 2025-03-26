use crate::buffer_content::{BufferContent, BufferStruct, Metadata};
use crate::descriptor::image_types::standard_image_types;
use crate::descriptor::reference::{AliveDescRef, Desc};
use crate::descriptor::{
	Buffer, BufferSlice, DescContent, DescriptorId, Image, ImageType, MutBuffer, MutBufferSlice, MutImage,
	TransientAccess, UnsafeDesc,
};
use bytemuck_derive::{Pod, Zeroable};
use spirv_std::{RuntimeArray, Sampler, TypedBuffer};

/// Some struct that facilitates access to a [`AliveDescRef`] pointing to some [`DescContent`]
pub trait DescriptorAccess<'a, C: DescContent> {
	type AccessType: 'a;

	fn access(self, desc: &Desc<impl AliveDescRef, C>) -> Self::AccessType;
}

macro_rules! decl_descriptors {
    ($($image:ident: $sampled:ident $storage:ident,)*) => {
		pub struct Descriptors<'a> {
			pub buffers: &'a RuntimeArray<TypedBuffer<[u32]>>,
			pub buffers_mut: &'a mut RuntimeArray<TypedBuffer<[u32]>>,
			$(
				pub $storage: &'a RuntimeArray<<crate::descriptor::$image as ImageType>::StorageSpvImage>,
				pub $sampled: &'a RuntimeArray<<crate::descriptor::$image as ImageType>::SampledSpvImage>,
			)*
			pub samplers: &'a RuntimeArray<Sampler>,
			pub meta: Metadata,
		}
		$(
			impl<'a> DescriptorAccess<'a, MutImage<crate::descriptor::$image>> for &'a Descriptors<'_> {
				type AccessType = &'a <crate::descriptor::$image as ImageType>::StorageSpvImage;

				fn access(self, desc: &Desc<impl AliveDescRef, MutImage<crate::descriptor::$image>>) -> Self::AccessType {
					unsafe { self.$storage.index(desc.id().index().to_usize()) }
				}
			}

			impl<'a> DescriptorAccess<'a, Image<crate::descriptor::$image>> for &'a Descriptors<'_> {
				type AccessType = &'a <crate::descriptor::$image as ImageType>::SampledSpvImage;

				fn access(self, desc: &Desc<impl AliveDescRef, Image<crate::descriptor::$image>>) -> Self::AccessType {
					unsafe { self.$sampled.index(desc.id().index().to_usize()) }
				}
			}
		)*
	};
}
standard_image_types!(decl_descriptors);

impl<'a, T: ?Sized + BufferContent + 'static> DescriptorAccess<'a, Buffer<T>> for &'a Descriptors<'_> {
	type AccessType = BufferSlice<'a, T>;

	fn access(self, desc: &Desc<impl AliveDescRef, Buffer<T>>) -> Self::AccessType {
		unsafe { BufferSlice::from_slice(self.buffers.index(desc.id().index().to_usize()), self.meta) }
	}
}

impl<'a, T: ?Sized + BufferContent + 'static> DescriptorAccess<'a, MutBuffer<T>> for &'a mut Descriptors<'_> {
	type AccessType = MutBufferSlice<'a, T>;

	fn access(self, desc: &Desc<impl AliveDescRef, MutBuffer<T>>) -> Self::AccessType {
		unsafe { MutBufferSlice::from_mut_slice(self.buffers_mut.index_mut(desc.id().index().to_usize()), self.meta) }
	}
}

impl<'a> DescriptorAccess<'a, Sampler> for &'a Descriptors<'_> {
	type AccessType = Sampler;

	fn access(self, desc: &Desc<impl AliveDescRef, Sampler>) -> Self::AccessType {
		unsafe { *self.samplers.index(desc.id().index().to_usize()) }
	}
}

unsafe impl<'a> TransientAccess<'a> for Descriptors<'a> {}

/// All bindless push constants are this particular struct, with T being the declared push_param.
///
/// Must not derive `DescStruct`, as to [`DescStruct::from_transfer`] Self you'd need the Metadata, which this struct
/// contains. To break the loop, it just stores Metadata flat and params directly as `T::TransferDescStruct`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Zeroable, Pod)]
pub struct BindlessPushConstant {
	param_desc: DescriptorId,
	param_offset: u32,
}

impl BindlessPushConstant {
	/// Construct the `PushConstant` struct.
	///
	/// # Safety
	/// `param_desc` at offset `param_offset` must contain valid parameters for the shader.
	pub unsafe fn new(param_desc: DescriptorId, param_offset: u32) -> Self {
		Self {
			param_desc,
			param_offset,
		}
	}

	pub fn load_param<T: BufferStruct>(&self, descriptors: &Descriptors) -> T {
		unsafe {
			UnsafeDesc::<Buffer<[u8]>>::new(self.param_desc)
				.to_transient_unchecked(descriptors)
				.access(descriptors)
				.load_at_arbitrary_offset_unchecked::<T>(self.param_offset as usize)
		}
	}
}
