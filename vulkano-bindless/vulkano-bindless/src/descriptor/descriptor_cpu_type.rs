use crate::atomic_slots::RCSlot;
use vulkano_bindless_shaders::descriptor::DescType;

pub trait DescCpuType: DescType {
	/// Type used within the [`RCSlot`]
	type TableType;
	/// CPU type exposed externally, that may contain extra generic type information
	type CpuType;

	/// deref [`Self::TableType`] to exposed [`Self::CpuType`]
	fn deref_table(slot: &RCSlot<Self::TableType>) -> &Self::CpuType;
}
