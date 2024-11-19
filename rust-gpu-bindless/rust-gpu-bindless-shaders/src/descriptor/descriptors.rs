use crate::buffer_content::{BufferContent, BufferStruct, Metadata};
use crate::descriptor::image_types::standard_image_types;
use crate::descriptor::reference::{AliveDescRef, Desc};
use crate::descriptor::{Buffer, BufferSlice, DescContent, DescriptorId, UnsafeDesc};
use bytemuck_derive::{Pod, Zeroable};
use spirv_std::{RuntimeArray, Sampler, TypedBuffer};

/// Some struct that facilitates access to a [`ValidDesc`] pointing to some [`DescContent`]
pub trait DescriptorsAccess<C: DescContent + ?Sized> {
	fn access(&self, desc: &Desc<impl AliveDescRef, C>) -> C::AccessType<'_>;
}

macro_rules! decl_descriptors {
    (
		{$($storage_name:ident: $storage_ty:ty,)*}
		{$($sampled_name:ident: $sampled_ty:ty,)*}
	) => {
		pub struct Descriptors<'a> {
			pub buffers: &'a RuntimeArray<TypedBuffer<[u32]>>,
			$(pub $storage_name: &'a RuntimeArray<$storage_ty>,)*
			$(pub $sampled_name: &'a RuntimeArray<$sampled_ty>,)*
			pub samplers: &'a RuntimeArray<Sampler>,
			pub meta: Metadata,
		}
		$(
			impl<'a> DescriptorsAccess<$storage_ty> for Descriptors<'a> {
				fn access(&self, desc: &Desc<impl AliveDescRef, $storage_ty>) -> <$storage_ty as DescContent>::AccessType<'_> {
					unsafe { self.$storage_name.index(desc.id().index().to_usize()) }
				}
			}
		)*
		$(
			impl<'a> DescriptorsAccess<$sampled_ty> for Descriptors<'a> {
				fn access(&self, desc: &Desc<impl AliveDescRef, $sampled_ty>) -> <$sampled_ty as DescContent>::AccessType<'_> {
					unsafe { self.$sampled_name.index(desc.id().index().to_usize()) }
				}
			}
		)*
	};
}
standard_image_types!(decl_descriptors);

impl<'a, T: ?Sized + BufferContent + 'static> DescriptorsAccess<Buffer<T>> for Descriptors<'a> {
	fn access(&self, desc: &Desc<impl AliveDescRef, Buffer<T>>) -> <Buffer<T> as DescContent>::AccessType<'_> {
		unsafe { BufferSlice::new(self.buffers.index(desc.id().index().to_usize()), self.meta) }
	}
}

impl<'a> DescriptorsAccess<Sampler> for Descriptors<'a> {
	fn access(&self, desc: &Desc<impl AliveDescRef, Sampler>) -> <Sampler as DescContent>::AccessType<'_> {
		unsafe { self.samplers.index(desc.id().index().to_usize()) }
	}
}

/// All bindless push constants are this particular struct, with T being the declared push_param.
///
/// Must not derive `DescStruct`, as to [`DescStruct::from_transfer`] Self you'd need the Metadata, which this struct
/// contains. To break the loop, it just stores Metadata flat and params directly as `T::TransferDescStruct`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Zeroable, Pod)]
pub struct BindlessPushConstant {
	param_desc: DescriptorId,
	param_offset: u32,
	pub metadata: Metadata,
}

impl BindlessPushConstant {
	/// Construct the `PushConstant` struct.
	///
	/// # Safety
	/// `param_desc` at offset `param_offset` must contain valid parameters for the shader.
	pub unsafe fn new(param_desc: DescriptorId, param_offset: u32, metadata: Metadata) -> Self {
		Self {
			param_desc,
			param_offset,
			metadata,
		}
	}

	pub fn load_param<T: BufferStruct>(&self, descriptors: &Descriptors) -> T {
		unsafe {
			UnsafeDesc::<Buffer<[u8]>>::new(self.param_desc)
				.to_transient_unchecked(descriptors.meta)
				.access(descriptors)
				.load_at_offset::<T>(self.param_offset as usize)
		}
	}
}
