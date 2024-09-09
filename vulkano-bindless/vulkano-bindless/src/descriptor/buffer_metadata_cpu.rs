use crate::descriptor::buffer_table::StrongBackingRefs;
use crate::descriptor::rc_reference::AnyRCDescExt;
use crate::descriptor::{AnyRCDesc, Bindless};
use ahash::{HashMap, HashMapExt};
use std::collections::hash_map::Entry;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use vulkano_bindless_shaders::buffer_content::{Metadata, MetadataCpuInterface};
use vulkano_bindless_shaders::descriptor::DescContent;
use vulkano_bindless_shaders::descriptor::DescContentType;
use vulkano_bindless_shaders::descriptor::StrongDesc;

/// Use as Metadata in [`DescStruct::write_cpu`] to figure out all [`StrongDesc`] contained within.
pub struct StrongMetadataCpu<'a> {
	bindless: &'a Arc<Bindless>,
	metadata: Metadata,
	refs: Result<HashMap<(DescContentType, u32), AnyRCDesc>, BackingRefsError>,
}

impl<'a> StrongMetadataCpu<'a> {
	/// See [`Self`]
	///
	/// # Safety
	/// You must call [`Self::into_backing_refs`] to actually retrieve the [`StrongBackingRefs`] before dropping this
	pub unsafe fn new(bindless: &'a Arc<Bindless>, metadata: Metadata) -> Self {
		Self {
			bindless,
			metadata,
			refs: Ok(HashMap::new()),
		}
	}

	pub fn into_backing_refs(self) -> Result<StrongBackingRefs, BackingRefsError> {
		Ok(StrongBackingRefs(self.refs?.into_values().collect()))
	}
}

unsafe impl<'a> MetadataCpuInterface for StrongMetadataCpu<'a> {
	fn visit_strong_descriptor<C: DescContent + ?Sized>(&mut self, desc: StrongDesc<C>) {
		if let Ok(refs) = &mut self.refs {
			let id = desc.id();
			let version = unsafe { desc.version_cpu() };
			match refs.entry((C::CONTENT_TYPE, desc.id())) {
				Entry::Occupied(rc) => {
					if rc.get().version() != version {
						self.refs = Err(BackingRefsError::NoLongerAlive(C::CONTENT_TYPE, id, version))
					}
				}
				Entry::Vacant(v) => {
					let rc = match C::CONTENT_TYPE {
						DescContentType::Buffer => self.bindless.buffer().resource_table.try_get_rc(id, version),
						DescContentType::Image => self.bindless.image().resource_table.try_get_rc(id, version),
						DescContentType::Sampler => self.bindless.sampler().resource_table.try_get_rc(id, version),
					};
					if let Some(rc) = rc {
						v.insert(rc);
					} else {
						self.refs = Err(BackingRefsError::NoLongerAlive(C::CONTENT_TYPE, id, version))
					}
				}
			}
		}
	}
}

impl<'a> Deref for StrongMetadataCpu<'a> {
	type Target = Metadata;

	fn deref(&self) -> &Self::Target {
		&self.metadata
	}
}

#[derive(Debug)]
pub enum BackingRefsError {
	NoLongerAlive(DescContentType, u32, u32),
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
