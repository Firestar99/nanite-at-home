use crate::descriptor::buffer_table::StrongBackingRefs;
use crate::descriptor::{AnyRCDesc, AnyRCDescExt, Bindless};
use ahash::{HashMap, HashMapExt};
use rust_gpu_bindless_shaders::buffer_content::{Metadata, MetadataCpuInterface};
use rust_gpu_bindless_shaders::descriptor::StrongDesc;
use rust_gpu_bindless_shaders::descriptor::{DescContent, DescriptorId};
use std::collections::hash_map::Entry;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;

/// Use as Metadata in [`DescStruct::write_cpu`] to figure out all [`StrongDesc`] contained within.
#[allow(dead_code)]
pub struct StrongMetadataCpu<'a> {
	bindless: &'a Arc<Bindless>,
	metadata: Metadata,
	refs: Result<HashMap<DescriptorId, AnyRCDesc>, BackingRefsError>,
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
			match refs.entry(id) {
				Entry::Occupied(_) => {}
				Entry::Vacant(v) => {
					if let Some(rc) = self.bindless.table_sync.try_recover(id) {
						v.insert(AnyRCDesc::new(rc));
					} else {
						self.refs = Err(BackingRefsError::NoLongerAlive(id))
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
	NoLongerAlive(DescriptorId),
}

impl Display for BackingRefsError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			BackingRefsError::NoLongerAlive(desc) => f.write_fmt(format_args!(
				"{:?} was no longer alive while StrongDesc of it existed",
				desc
			)),
		}
	}
}
