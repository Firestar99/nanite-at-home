use crate::descriptor::buffer_table::StrongBackingRefs;
use crate::descriptor::{AnyRCDesc, Bindless};
use ahash::{HashMap, HashMapExt};
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use vulkano_bindless_shaders::buffer_content::{Metadata, MetadataCpuInterface};
use vulkano_bindless_shaders::descriptor::DescContent;
use vulkano_bindless_shaders::descriptor::DescContentType;
use vulkano_bindless_shaders::descriptor::StrongDesc;

/// Use as Metadata in [`DescStruct::write_cpu`] to figure out all [`StrongDesc`] contained within.
#[allow(dead_code)]
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
	fn visit_strong_descriptor<C: DescContent + ?Sized>(&mut self, _desc: StrongDesc<C>) {
		todo!()
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
