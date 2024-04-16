use crate::descriptor::DescType;

pub trait ValidDesc<D: DescType> {
	fn id(&self) -> u32;
}
