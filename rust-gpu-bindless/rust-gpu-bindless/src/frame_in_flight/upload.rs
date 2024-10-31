use crate::descriptor::buffer_table::StrongBackingRefs;
use crate::descriptor::rc::RCDescExt;
use crate::descriptor::{Bindless, RCDesc};
use crate::frame_in_flight::{FrameInFlight, ResourceInFlight, SeedInFlight};
use rust_gpu_bindless_shaders::buffer_content::{BufferStruct, Metadata, MetadataCpuInterface};
use rust_gpu_bindless_shaders::descriptor::{Buffer, DescContent, StrongDesc, TransientDesc};
use std::alloc::Layout;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::buffer::{BufferContents, BufferMemory};
use vulkano::buffer::{BufferCreateInfo, BufferUsage};
use vulkano::device::DeviceOwned;
use vulkano::memory::allocator::{AllocationCreateInfo, DeviceLayout, MemoryAllocator, MemoryTypeFilter};
use vulkano::memory::{MappedMemoryRange, MemoryPropertyFlags};
use vulkano::{Validated, VulkanError};

pub struct UploadInFlight<T: BufferStruct + 'static> {
	sub: ResourceInFlight<RCDesc<Buffer<T>>>,
}

impl<T: BufferStruct> UploadInFlight<T> {
	pub fn new(
		bindless: &Arc<Bindless>,
		allocator: Arc<dyn MemoryAllocator>,
		seed: impl Into<SeedInFlight>,
		usage: BufferUsage,
	) -> Self {
		UploadInFlight {
			sub: ResourceInFlight::new(seed, |_| {
				bindless
					.buffer()
					.alloc_sized(
						allocator.clone(),
						BufferCreateInfo {
							usage,
							..BufferCreateInfo::default()
						},
						AllocationCreateInfo {
							memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
								| MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
							..AllocationCreateInfo::default()
						},
						StrongBackingRefs::default(),
					)
					.unwrap()
			}),
		}
	}

	/// Upload some data to that uniform
	///
	/// # Safety
	/// You must upload data before using it and may not upload anything after launching draws or dispatches
	pub unsafe fn upload<'a>(
		&'a self,
		fif: FrameInFlight<'a>,
		data: T,
	) -> Result<TransientDesc<'a, Buffer<T>>, Validated<VulkanError>> {
		let sub = self.sub.index(fif);
		// a normal write won't work as vulkano thinks the buffer is constantly accessed by the GPU
		// *sub.write().unwrap() = data;

		unsafe {
			{
				let mapped = <T::Transfer as BufferContents>::ptr_from_slice(sub.mapped_slice().unwrap());
				*mapped = data.write_cpu(&mut UniformMetadataCpu(Metadata));
			}

			let allocation = match sub.buffer().memory() {
				BufferMemory::Normal(a) => a,
				_ => unreachable!(),
			};
			let is_coherent = sub.device().physical_device().memory_properties().memory_types
				[allocation.device_memory().memory_type_index() as usize]
				.property_flags
				.contains(MemoryPropertyFlags::HOST_COHERENT);
			if !is_coherent {
				let atom_size = sub.device().physical_device().properties().non_coherent_atom_size;
				let layout = DeviceLayout::from_layout(Layout::new::<T::Transfer>()).unwrap();
				let size = layout.align_to(atom_size).unwrap().pad_to_alignment().size();
				allocation.flush_range(MappedMemoryRange {
					offset: sub.offset(),
					size,
					..MappedMemoryRange::default()
				})?;
			}
			Ok(sub.to_transient(fif))
		}
	}

	pub fn seed(&self) -> SeedInFlight {
		self.sub.seed()
	}
}

impl<T: BufferStruct> From<&UploadInFlight<T>> for SeedInFlight {
	fn from(value: &UploadInFlight<T>) -> Self {
		value.seed()
	}
}

struct UniformMetadataCpu(Metadata);

impl Deref for UniformMetadataCpu {
	type Target = Metadata;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

unsafe impl MetadataCpuInterface for UniformMetadataCpu {
	fn visit_strong_descriptor<C: DescContent + ?Sized>(&mut self, _desc: StrongDesc<C>) {
		// don't care, will be alive for this fif anyway
	}
}
