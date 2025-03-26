use crate::backing::table::RcTableSlot;
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::descriptor::DescContent;

/// A descriptor type to some resource, that may have generic arguments to specify its contents.
pub trait DescContentCpu: DescContent {
	/// Associated non-generic [`DescTable`]
	type DescTable<P: BindlessPlatform>: DescTable<P>;
}

/// In a resource table descriptors of varying generic arguments can be stored and are sent to the GPU in a single descriptor binding.
pub trait DescTable<P: BindlessPlatform>: Sized {
	/// The kind of slot
	type Slot;

	/// get the slot from an [`RcTableSlot`] that *should* point to a table of that slot type
	fn get_slot(slot: &RcTableSlot) -> &Self::Slot;
}

pub trait DescContentMutCpu: DescContentCpu {
	type Shared: DescContentCpu;
	type Access: Copy + Eq + Send + Sync + 'static;
}
