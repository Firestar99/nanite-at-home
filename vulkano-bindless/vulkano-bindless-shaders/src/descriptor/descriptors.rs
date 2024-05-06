use crate::descriptor::SampledImage2D;
use spirv_std::{RuntimeArray, Sampler};

pub struct Descriptors<'a> {
	pub(crate) buffers: &'a mut RuntimeArray<[u32]>,
	pub(crate) sampled_images_2d: &'a RuntimeArray<SampledImage2D>,
	pub(crate) samplers: &'a RuntimeArray<Sampler>,
}

impl<'a> Descriptors<'a> {
	pub fn new(
		buffers: &'a mut RuntimeArray<[u32]>,
		sampled_images_2d: &'a RuntimeArray<SampledImage2D>,
		samplers: &'a RuntimeArray<Sampler>,
	) -> Descriptors<'a> {
		Self {
			buffers,
			sampled_images_2d,
			samplers,
		}
	}
}
