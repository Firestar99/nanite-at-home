use crate::desc_buffer::DescBuffer;
use crate::descriptor::image_types::standard_image_types;
use crate::descriptor::metadata::Metadata;
use crate::descriptor::reference::{AliveDescRef, Desc};
use crate::descriptor::{Buffer, BufferSlice, DescContent};
use spirv_std::{RuntimeArray, Sampler};

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
			pub buffers: &'a mut RuntimeArray<[u32]>,
			$(pub $storage_name: &'a RuntimeArray<$storage_ty>,)*
			$(pub $sampled_name: &'a RuntimeArray<$sampled_ty>,)*
			pub samplers: &'a RuntimeArray<Sampler>,
			pub meta: Metadata,
		}
		$(
			impl<'a> DescriptorsAccess<$storage_ty> for Descriptors<'a> {
				fn access(&self, desc: &Desc<impl AliveDescRef, $storage_ty>) -> <$storage_ty as DescContent>::AccessType<'_> {
					unsafe { self.$storage_name.index(desc.id() as usize) }
				}
			}
		)*
		$(
			impl<'a> DescriptorsAccess<$sampled_ty> for Descriptors<'a> {
				fn access(&self, desc: &Desc<impl AliveDescRef, $sampled_ty>) -> <$sampled_ty as DescContent>::AccessType<'_> {
					unsafe { self.$sampled_name.index(desc.id() as usize) }
				}
			}
		)*
	};
}
standard_image_types!(decl_descriptors);

impl<'a, T: ?Sized + DescBuffer + 'static> DescriptorsAccess<Buffer<T>> for Descriptors<'a> {
	fn access(&self, desc: &Desc<impl AliveDescRef, Buffer<T>>) -> <Buffer<T> as DescContent>::AccessType<'_> {
		unsafe { BufferSlice::new(self.buffers.index(desc.id() as usize), self.meta) }
	}
}

impl<'a> DescriptorsAccess<Sampler> for Descriptors<'a> {
	fn access(&self, desc: &Desc<impl AliveDescRef, Sampler>) -> <Sampler as DescContent>::AccessType<'_> {
		unsafe { self.samplers.index(desc.id() as usize) }
	}
}
