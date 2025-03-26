use crate::backing::table::{Table, TableInterface};
use rangemap::RangeSet;
use rust_gpu_bindless_shaders::descriptor::DescriptorIndex;
use std::ops::Range;

pub fn range_to_descriptor_index(range: Range<DescriptorIndex>) -> impl Iterator<Item = DescriptorIndex> {
	(range.start.to_u32()..range.end.to_u32()).map(|i| unsafe { DescriptorIndex::new(i).unwrap() })
}

pub(crate) fn descriptor_index_to_range(index: DescriptorIndex) -> Range<DescriptorIndex> {
	unsafe { index..DescriptorIndex::new(index.to_u32() + 1).unwrap() }
}

pub trait DescriptorIndexIterator<'a, I: TableInterface>: Sized {
	fn into_inner(self) -> (&'a Table<I>, impl Iterator<Item = DescriptorIndex>);

	fn into_iter(self) -> impl Iterator<Item = (DescriptorIndex, &'a I::Slot)> {
		// Safety: indices are guaranteed to be alive by constructor
		unsafe {
			let (table, iter) = self.into_inner();
			iter.map(|i| (i, table.get_slot_unchecked(i)))
		}
	}

	fn into_vec(self) -> Vec<(DescriptorIndex, &'a I::Slot)> {
		self.into_iter().collect()
	}

	fn into_range_set(self) -> DescriptorIndexRangeSet<'a, Table<I>> {
		// Safety: indices are guaranteed to be alive by constructor
		let (table, iter) = self.into_inner();
		DescriptorIndexRangeSet {
			range_set: iter.map(descriptor_index_to_range).collect(),
			table,
		}
	}
}

#[derive(Debug)]
pub struct DescriptorIndexRangeSet<'a, T> {
	range_set: RangeSet<DescriptorIndex>,
	table: &'a T,
}

impl<T> Clone for DescriptorIndexRangeSet<'_, T> {
	fn clone(&self) -> Self {
		Self {
			table: self.table,
			range_set: self.range_set.clone(),
		}
	}
}

impl<'a, T> DescriptorIndexRangeSet<'a, T> {
	/// # Safety
	/// indices must be alive and match the table
	pub unsafe fn from(table: &'a T, iter: impl Iterator<Item = DescriptorIndex>) -> Self {
		Self {
			range_set: iter.map(descriptor_index_to_range).collect(),
			table,
		}
	}

	/// # Safety
	/// indices must be alive and match the table
	pub unsafe fn new(table: &'a T, range_set: RangeSet<DescriptorIndex>) -> Self {
		Self { table, range_set }
	}

	/// # Safety
	/// indices must be alive and match the table
	pub unsafe fn insert(&mut self, index: DescriptorIndex) {
		self.range_set.insert(descriptor_index_to_range(index));
	}

	pub fn is_empty(&self) -> bool {
		self.range_set.is_empty()
	}

	pub fn table(&self) -> &'a T {
		self.table
	}

	pub fn into_range_set(self) -> RangeSet<DescriptorIndex> {
		self.range_set
	}
}

impl<I: TableInterface> DescriptorIndexRangeSet<'_, Table<I>> {
	pub fn iter_ranges(
		&self,
	) -> impl Iterator<
		Item = (
			Range<DescriptorIndex>,
			impl Iterator<Item = (DescriptorIndex, &I::Slot)>,
		),
	> + '_ {
		// Safety: indices are guaranteed to be alive by constructor
		unsafe {
			self.range_set.iter().map(|range| {
				(
					range.clone(),
					range_to_descriptor_index(range.clone()).map(|i| (i, self.table.get_slot_unchecked(i))),
				)
			})
		}
	}

	pub fn iter(&self) -> impl Iterator<Item = (DescriptorIndex, &I::Slot)> + '_ {
		// Safety: indices are guaranteed to be alive by constructor
		unsafe {
			self.range_set
				.iter()
				.cloned()
				.flat_map(range_to_descriptor_index)
				.map(|i| (i, self.table.get_slot_unchecked(i)))
		}
	}
}

impl<'a, I: TableInterface> DescriptorIndexIterator<'a, I> for DescriptorIndexRangeSet<'a, Table<I>> {
	fn into_inner(self) -> (&'a Table<I>, impl Iterator<Item = DescriptorIndex>) {
		(
			self.table,
			self.range_set.into_iter().flat_map(range_to_descriptor_index),
		)
	}

	fn into_range_set(self) -> DescriptorIndexRangeSet<'a, Table<I>> {
		self
	}
}

impl<'a, I: TableInterface> DescriptorIndexIterator<'a, I> for &DescriptorIndexRangeSet<'a, Table<I>> {
	fn into_inner(self) -> (&'a Table<I>, impl Iterator<Item = DescriptorIndex>) {
		(
			self.table,
			self.range_set.iter().cloned().flat_map(range_to_descriptor_index),
		)
	}

	fn into_range_set(self) -> DescriptorIndexRangeSet<'a, Table<I>> {
		self.clone()
	}
}
