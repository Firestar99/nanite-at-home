use vulkano::buffer::Subbuffer;
use vulkano_bindless_shaders::descriptor::{BufferType, DescType};

pub trait DescTableType: DescType {
	type CpuType;
}

impl<T: ?Sized> DescTableType for BufferType<T> {
	type CpuType = Subbuffer<[u8]>;
}
