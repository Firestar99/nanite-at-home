use crate::backend::table::TableSync;
use crate::descriptor::buffer_table::StrongBackingRefs;
use crate::descriptor::{AnyRCDesc, AnyRCDescExt};
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::buffer_content::{Metadata, MetadataCpuInterface};
use rust_gpu_bindless_shaders::descriptor::StrongDesc;
use rust_gpu_bindless_shaders::descriptor::{DescContent, DescriptorId};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;

// TODO I don't like that this can dynamically error and would like it gone. Instead we could have an UploadContext that
//  allows you to convert RCDesc to StrongDesc and already (uniquely) clone RCs when the StrongDesc are created. This
//  ensures that there is never the possibility of StrongDesc becoming invalid between creation and upload. It would
//  require Strong to have a lifetime on UploadContext though to ensure at compile time it doesn't escape. Also one may
//  not mix different UploadContexts, though I think that one could be a runtime check given how rare that is. Or one
//  could use thread-local storage and a reentrant-lock like structure.

/// Use as Metadata in [`DescStruct::write_cpu`] to figure out all [`StrongDesc`] contained within.
#[allow(dead_code)]
pub struct StrongMetadataCpu<'a, P: BindlessPlatform> {
	table_sync: &'a Arc<TableSync>,
	metadata: Metadata,
	refs: Result<HashMap<DescriptorId, AnyRCDesc<P>>, BackingRefsError>,
}

impl<'a, P: BindlessPlatform> StrongMetadataCpu<'a, P> {
	/// See [`Self`]
	///
	/// # Safety
	/// You must call [`Self::into_backing_refs`] to actually retrieve the [`StrongBackingRefs`] before dropping this
	pub unsafe fn new(table_sync: &'a Arc<TableSync>, metadata: Metadata) -> Self {
		Self {
			table_sync,
			metadata,
			refs: Ok(HashMap::new()),
		}
	}

	pub fn into_backing_refs(self) -> StrongBackingRefs<P> {
		StrongBackingRefs(self.refs.expect("BackingRefsError occurred:").into_values().collect())
	}
}

unsafe impl<'a, P: BindlessPlatform> MetadataCpuInterface for StrongMetadataCpu<'a, P> {
	fn visit_strong_descriptor<C: DescContent + ?Sized>(&mut self, desc: StrongDesc<C>) {
		if let Ok(refs) = &mut self.refs {
			let id = desc.id();
			match refs.entry(id) {
				Entry::Occupied(_) => {}
				Entry::Vacant(v) => {
					if let Some(rc) = self.table_sync.try_recover(id) {
						v.insert(AnyRCDesc::new(rc));
					} else {
						self.refs = Err(BackingRefsError::NoLongerAlive(id))
					}
				}
			}
		}
	}
}

impl<'a, P: BindlessPlatform> Deref for StrongMetadataCpu<'a, P> {
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
