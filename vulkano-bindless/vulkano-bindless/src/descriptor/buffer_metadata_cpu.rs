use crate::descriptor::rc_reference::AnyRCDesc;
use crate::descriptor::{Bindless, BufferTable, ImageTable, SamplerTable};
use smallvec::SmallVec;
use std::collections::HashSet;
use std::ops::Deref;
use std::sync::Arc;
use vulkano_bindless_shaders::desc_buffer::MetadataCpuInterface;
use vulkano_bindless_shaders::descriptor::descriptor_type::DescEnum;
use vulkano_bindless_shaders::descriptor::metadata::Metadata;
use vulkano_bindless_shaders::descriptor::reference::StrongDesc;
use vulkano_bindless_shaders::descriptor::{DescType, ValidDesc};

/// Stores [`AnyRCDesc`] to various resources, to which [`StrongDesc`] contained in some resource may refer to.
#[derive(Clone)]
pub struct StrongBackingRefs {
	_buffer: SmallVec<[AnyRCDesc<BufferTable>; 4]>,
	_image: SmallVec<[AnyRCDesc<ImageTable>; 4]>,
	_sampler: SmallVec<[AnyRCDesc<SamplerTable>; 1]>,
}

/// Use as Metadata in [`DescStruct::write_cpu`] to figure out all [`StrongDesc`] contained within.
pub struct StrongMetadataCpu {
	metadata: Metadata,
	buffer: HashSet<u32>,
	image: HashSet<u32>,
	sampler: HashSet<u32>,
}

impl StrongMetadataCpu {
	/// See [`Self`]
	///
	/// # Safety
	/// You must call [`Self::into_backing_refs`] to actually retrieve the [`StrongBackingRefs`] before dropping this
	pub unsafe fn new(metadata: Metadata) -> Self {
		Self {
			metadata,
			buffer: HashSet::new(),
			image: HashSet::new(),
			sampler: HashSet::new(),
		}
	}

	#[must_use]
	pub fn into_backing_refs(self, bindless: &Arc<Bindless>) -> StrongBackingRefs {
		StrongBackingRefs {
			_buffer: self
				.buffer
				.into_iter()
				.map(|id| {
					bindless
						.buffer
						.resource_table
						.try_get_rc(id)
						.ok_or_else(|| format!("Buffer {} was no longer alive while StrongDesc of it existed", id))
				})
				.collect::<Result<_, String>>()
				.unwrap(),
			_image: self
				.image
				.into_iter()
				.map(|id| {
					bindless
						.image
						.resource_table
						.try_get_rc(id)
						.ok_or_else(|| format!("Image {} was no longer alive while StrongDesc of it existed", id))
				})
				.collect::<Result<_, String>>()
				.unwrap(),
			_sampler: self
				.sampler
				.into_iter()
				.map(|id| {
					bindless
						.sampler
						.resource_table
						.try_get_rc(id)
						.ok_or_else(|| format!("Sampler {} was no longer alive while StrongDesc of it existed", id))
				})
				.collect::<Result<_, String>>()
				.unwrap(),
		}
	}
}

unsafe impl MetadataCpuInterface for StrongMetadataCpu {
	fn visit_strong_descriptor<D: DescType + ?Sized>(&mut self, desc: StrongDesc<'_, D>) {
		match D::DESC_ENUM {
			DescEnum::Buffer => &mut self.buffer,
			DescEnum::Image => &mut self.image,
			DescEnum::Sampler => &mut self.sampler,
		}
		.insert(desc.id());
	}
}

impl Deref for StrongMetadataCpu {
	type Target = Metadata;

	fn deref(&self) -> &Self::Target {
		&self.metadata
	}
}
