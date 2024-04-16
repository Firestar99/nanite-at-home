use crate::descriptor::DescType;

pub struct Descriptors<'a> {
	pub buffer_data: &'a mut [&'a mut [u32]],
}

impl<'a> Descriptors<'a> {
	pub fn new(buffer_data: &'a mut [&'a mut [u32]]) -> Descriptors<'a> {
		Self { buffer_data }
	}
}

pub trait AccessibleDesc {
	fn access<'a, D: DescType>(d: &'a Descriptors) -> D::AccessType<'a>;
}
