use crate::descriptor::buffer_table::BufferResourceTable;
use std::marker::PhantomData;
use std::sync::Arc;
use vulkano::device::Device;

pub struct Descriptors {
	pub device: Arc<Device>,
	pub buffer: BufferResourceTable,
	_private: PhantomData<()>,
}

impl Descriptors {
	/// Creates a new Descriptors instance with which to allocate descriptors.
	///
	/// # Safety
	/// There must only be one global Descriptors instance for each [`Device`].
	pub unsafe fn new(device: Arc<Device>) -> Self {
		Self {
			buffer: BufferResourceTable::new(device.clone()),
			device,
			_private: PhantomData {},
		}
	}

	pub fn flush() {}
}
