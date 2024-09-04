use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::resource_table::TableEpochGuard;
use crate::descriptor::{BufferTable, ImageTable, SamplerTable};
use crate::rc_slot::RCSlotsInterface;
use std::collections::BTreeMap;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{DescriptorBindingFlags, DescriptorSetLayoutBinding};
use vulkano::device::physical::PhysicalDevice;
use vulkano::shader::ShaderStages;
use vulkano_bindless_shaders::descriptor::{DescContent, DescContentEnum};

/// A descriptor type to some resource, that may have generic arguments to specify its contents.
pub trait DescContentCpu: DescContent {
	/// Associated non-generic [`DescTable`]
	type DescTable: DescTable;

	/// CPU type exposed externally, that may contain extra generic type information
	type VulkanType;

	/// deref [`Self::TableType`] to exposed [`Self::VulkanType`]
	fn deref_table(slot: &<Self::DescTable as DescTable>::Slot) -> &Self::VulkanType;
}

/// In a resource table descriptors of varying generic arguments can be stored and are sent to the GPU in a single descriptor binding.
pub trait DescTable: Sized {
	const CONTENT_ENUM: DescContentEnum;
	/// internal non-generic type used within the resource table
	type Slot;
	type RCSlotsInterface: RCSlotsInterface<Self::Slot>;

	fn max_update_after_bind_descriptors(physical_device: &Arc<PhysicalDevice>) -> u32;

	const BINDING_FLAGS: DescriptorBindingFlags = DescriptorBindingFlags::UPDATE_AFTER_BIND
		.union(DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING)
		.union(DescriptorBindingFlags::PARTIALLY_BOUND);

	fn layout_binding(
		stages: ShaderStages,
		count: DescriptorCounts,
		out: &mut BTreeMap<u32, DescriptorSetLayoutBinding>,
	);

	fn lock_table(&self) -> TableEpochGuard<Self>;

	fn table_enum_new<A: DescTableEnumType>(inner: A::Type<Self>) -> DescTableEnum<A>;

	fn table_enum_try_deref<A: DescTableEnumType>(table_enum: &DescTableEnum<A>) -> Option<&A::Type<Self>>;

	fn table_enum_try_into<A: DescTableEnumType>(
		table_enum: DescTableEnum<A>,
	) -> Result<A::Type<Self>, DescTableEnum<A>>;
}

/// An enum of the kind of descriptor. Get it for any generic descriptor via [`DescContent::CONTENT_ENUM`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum DescTableEnum<A: DescTableEnumType> {
	Buffer(A::Type<BufferTable>),
	Image(A::Type<ImageTable>),
	Sampler(A::Type<SamplerTable>),
}

pub trait DescTableEnumType {
	type Type<T: DescTable>;
}

impl<A: DescTableEnumType> DescTableEnum<A> {
	#[inline]
	pub fn new<T: DescTable>(inner: A::Type<T>) -> Self {
		T::table_enum_new(inner)
	}

	#[inline]
	pub fn try_deref<T: DescTable>(&self) -> Option<&A::Type<T>> {
		T::table_enum_try_deref(self)
	}

	#[inline]
	pub fn try_into<T: DescTable>(self) -> Result<A::Type<T>, Self> {
		T::table_enum_try_into(self)
	}
}
