use crate::descriptor::{Buffer, BufferSlice, DescType, ValidDesc};
use macros::image::Image2d;
use spirv_std::{RuntimeArray, Sampler};

pub trait DescriptorsAccess<D: DescType + ?Sized> {
	fn access(&self, desc: &impl ValidDesc<D>) -> D::AccessType<'_>;
}

pub struct Descriptors<'a> {
	pub(crate) buffers: &'a mut RuntimeArray<[u32]>,
	pub(crate) sampled_images_2d: &'a RuntimeArray<Image2d>,
	pub(crate) samplers: &'a RuntimeArray<Sampler>,
}

impl<'a> Descriptors<'a> {
	pub fn new(
		buffers: &'a mut RuntimeArray<[u32]>,
		sampled_images_2d: &'a RuntimeArray<Image2d>,
		samplers: &'a RuntimeArray<Sampler>,
	) -> Descriptors<'a> {
		Self {
			buffers,
			sampled_images_2d,
			samplers,
		}
	}
}

impl<'a, T: ?Sized + 'static> DescriptorsAccess<Buffer<T>> for Descriptors<'a> {
	fn access(&self, desc: &impl ValidDesc<Buffer<T>>) -> <Buffer<T> as DescType>::AccessType<'_> {
		BufferSlice::new(unsafe { self.buffers.index(desc.id() as usize) })
	}
}

impl<'a> DescriptorsAccess<Sampler> for Descriptors<'a> {
	fn access(&self, desc: &impl ValidDesc<Sampler>) -> <Sampler as DescType>::AccessType<'_> {
		unsafe { self.samplers.index(desc.id() as usize) }
	}
}

impl<'a> DescriptorsAccess<Image2d> for Descriptors<'a> {
	fn access(&self, desc: &impl ValidDesc<Image2d>) -> <Image2d as DescType>::AccessType<'_> {
		unsafe { self.sampled_images_2d.index(desc.id() as usize) }
	}
}
