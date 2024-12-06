use crate::buffer_content::{BufferContent, BufferStruct, Metadata};
use crate::descriptor::image_types::standard_image_types;
use crate::descriptor::reference::{AliveDescRef, Desc};
use crate::descriptor::{
	Buffer, BufferSlice, DescContent, DescriptorId, MutBuffer, MutBufferSlice, TransientAccess, UnsafeDesc,
};
use bytemuck_derive::{Pod, Zeroable};
use spirv_std::{RuntimeArray, Sampler, TypedBuffer};

/// Some struct that facilitates access to a [`AliveDescRef`] pointing to some [`DescContent`]
pub trait DescriptorAccess<'a, C: DescContent + ?Sized> {
	type AccessType: 'a;

	fn access(&'a self, desc: &Desc<impl AliveDescRef, C>) -> Self::AccessType;
}

/// Some struct that facilitates access to a [`AliveDescRef`] pointing to some [`DescContent`]
pub trait DescriptorAccessMut<'a, C: DescContent + ?Sized> {
	type AccessTypeMut: 'a;

	fn access_mut(&'a mut self, desc: &Desc<impl AliveDescRef, C>) -> Self::AccessTypeMut;
}

macro_rules! decl_descriptors {
    (
		{$($storage_name:ident: $storage_ty:ty,)*}
		{$($sampled_name:ident: $sampled_ty:ty,)*}
	) => {
		pub struct Descriptors<'a> {
			pub buffers: &'a RuntimeArray<TypedBuffer<[u32]>>,
			pub buffers_mut: &'a mut RuntimeArray<TypedBuffer<[u32]>>,
			$(pub $storage_name: &'a RuntimeArray<$storage_ty>,)*
			$(pub $sampled_name: &'a RuntimeArray<$sampled_ty>,)*
			pub samplers: &'a RuntimeArray<Sampler>,
			pub meta: Metadata,
		}
		$(
			impl<'a> DescriptorAccess<'a, $storage_ty> for Descriptors<'a> {
				type AccessType = &'a $storage_ty;

				fn access(&'a self, desc: &Desc<impl AliveDescRef, $storage_ty>) -> Self::AccessType {
					unsafe { self.$storage_name.index(desc.id().index().to_usize()) }
				}
			}
		)*
		$(
			impl<'a> DescriptorAccess<'a, $sampled_ty> for Descriptors<'a> {
				type AccessType = &'a $sampled_ty;

				fn access(&'a self, desc: &Desc<impl AliveDescRef, $sampled_ty>) -> Self::AccessType {
					unsafe { self.$sampled_name.index(desc.id().index().to_usize()) }
				}
			}
		)*
	};
}
standard_image_types!(decl_descriptors);

impl<'a, T: ?Sized + BufferContent + 'static> DescriptorAccess<'a, Buffer<T>> for Descriptors<'a> {
	type AccessType = BufferSlice<'a, T>;

	fn access(&'a self, desc: &Desc<impl AliveDescRef, Buffer<T>>) -> Self::AccessType {
		unsafe { BufferSlice::from_slice(self.buffers.index(desc.id().index().to_usize()), self.meta) }
	}
}

impl<'a, T: ?Sized + BufferContent + 'static> DescriptorAccessMut<'a, MutBuffer<T>> for Descriptors<'a> {
	type AccessTypeMut = MutBufferSlice<'a, T>;

	fn access_mut(&'a mut self, desc: &Desc<impl AliveDescRef, MutBuffer<T>>) -> Self::AccessTypeMut {
		unsafe { MutBufferSlice::from_mut_slice(self.buffers_mut.index_mut(desc.id().index().to_usize()), self.meta) }
	}
}

impl<'a> DescriptorAccess<'a, Sampler> for Descriptors<'a> {
	type AccessType = Sampler;

	fn access(&'a self, desc: &Desc<impl AliveDescRef, Sampler>) -> Self::AccessType {
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
