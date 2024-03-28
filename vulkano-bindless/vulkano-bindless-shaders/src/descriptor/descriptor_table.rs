use spirv_std::{Image, RuntimeArray};

pub struct DescriptorTable<'a, T> {
	descriptor_table: &'a RuntimeArray<T>,
}
