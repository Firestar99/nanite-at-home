use crate::descriptor::rc_reference::AnyRCDesc;
use crate::descriptor::{Bindless, BufferTable, DescTable, ImageTable, ResourceTable, SamplerTable};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
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
	buffer: HashMap<u32, u32>,
	image: HashMap<u32, u32>,
	sampler: HashMap<u32, u32>,
}

impl StrongMetadataCpu {
	/// See [`Self`]
	///
	/// # Safety
	/// You must call [`Self::into_backing_refs`] to actually retrieve the [`StrongBackingRefs`] before dropping this
	pub unsafe fn new(metadata: Metadata) -> Self {
		Self {
			metadata,
			buffer: HashMap::new(),
			image: HashMap::new(),
			sampler: HashMap::new(),
		}
	}

	pub fn into_backing_refs(self, bindless: &Arc<Bindless>) -> Result<StrongBackingRefs, BackingRefsError> {
		fn convert<T: DescTable, B: FromIterator<AnyRCDesc<T>>>(
			hash_map: HashMap<u32, u32>,
			resource_table: &ResourceTable<T>,
		) -> Result<B, BackingRefsError> {
			hash_map
				.into_iter()
				.map(|(id, version)| {
					resource_table
						.try_get_rc(id, version)
						.ok_or(BackingRefsError::NoLongerAlive(T::DESC_ENUM, id, version))
				})
				.collect()
		}
		Ok(StrongBackingRefs {
			_buffer: convert(self.buffer, &bindless.buffer.resource_table)?,
			_image: convert(self.image, &bindless.image.resource_table)?,
			_sampler: convert(self.sampler, &bindless.sampler.resource_table)?,
		})
	}
}

unsafe impl MetadataCpuInterface for StrongMetadataCpu {
	fn visit_strong_descriptor<D: DescType + ?Sized>(&mut self, desc: StrongDesc<D>) {
		// Safety: we are on CPU
		let version = unsafe { desc.version_cpu() };
		match D::DESC_ENUM {
			DescEnum::Buffer => &mut self.buffer,
			DescEnum::Image => &mut self.image,
			DescEnum::Sampler => &mut self.sampler,
		}
		.insert(desc.id(), version);
	}
}

impl Deref for StrongMetadataCpu {
	type Target = Metadata;

	fn deref(&self) -> &Self::Target {
		&self.metadata
	}
}

pub enum BackingRefsError {
	NoLongerAlive(DescEnum, u32, u32),
}

impl Display for BackingRefsError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			BackingRefsError::NoLongerAlive(desc, id, version) => f.write_fmt(format_args!(
				"{:?} id: {} version: {} was no longer alive while StrongDesc of it existed",
				desc, id, version
			)),
		}
	}
}
