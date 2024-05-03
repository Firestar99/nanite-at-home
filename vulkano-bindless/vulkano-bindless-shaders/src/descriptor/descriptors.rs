use spirv_std::RuntimeArray;

pub struct Descriptors<'a> {
	pub(crate) buffer_data: &'a mut RuntimeArray<[u32]>,
}

impl<'a> Descriptors<'a> {
	pub fn new(buffer_data: &'a mut RuntimeArray<[u32]>) -> Descriptors<'a> {
		Self { buffer_data }
	}
}
