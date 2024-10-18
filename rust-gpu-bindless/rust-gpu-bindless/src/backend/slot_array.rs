use rust_gpu_bindless_shaders::descriptor::DescriptorIndex;
use std::ops::{Index, IndexMut};

pub struct SlotArray<T>(pub Box<[T]>);

impl<T: Default> SlotArray<T> {
	pub fn new(count: u32) -> Self {
		Self::new_generator(count, |_| T::default())
	}
}

impl<T> SlotArray<T> {
	pub fn new_generator(count: u32, f: impl FnMut(u32) -> T) -> Self {
		Self((0..count).map(f).collect::<Vec<_>>().into_boxed_slice())
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}
}

impl<T> Index<DescriptorIndex> for SlotArray<T> {
	type Output = T;

	fn index(&self, index: DescriptorIndex) -> &Self::Output {
		self.0.index(index.to_usize())
	}
}

impl<T> IndexMut<DescriptorIndex> for SlotArray<T> {
	fn index_mut(&mut self, index: DescriptorIndex) -> &mut Self::Output {
		self.0.index_mut(index.to_usize())
	}
}
