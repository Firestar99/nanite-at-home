pub trait ValidRef {
	fn id(&self) -> u32;
}

pub trait ValidBufferReference<T>: ValidRef {
	fn access(&self) -> &T;
	fn access_mut(&mut self) -> &mut T;
}
