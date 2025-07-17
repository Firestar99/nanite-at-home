use crate::descriptor::{
	BindlessAllocationScheme, BindlessBufferUsage, BufferAllocationError, BufferSlot, BufferTableAccess, MutDesc,
	RCDesc, Sampler, SamplerAllocationError, SamplerTableAccess,
};
use crate::pipeline::{AccessLock, BufferAccess};
use crate::platform::ash::{Ash, AshAllocationError, AshBuffer, AshMemoryAllocation};
use ash::vk::{DebugUtilsObjectNameInfoEXT, SamplerCreateInfo};
use gpu_allocator::MemoryLocation;
use gpu_allocator::vulkan::AllocationCreateDesc;
use rust_gpu_bindless_shaders::buffer_content::BufferContent;
use rust_gpu_bindless_shaders::descriptor::MutBuffer;
use std::ffi::CString;

impl SamplerTableAccess<'_, Ash> {
	pub fn alloc_ash(
		&self,
		sampler_create_info: &SamplerCreateInfo,
	) -> Result<RCDesc<Ash, Sampler>, SamplerAllocationError<Ash>> {
		unsafe {
			Ok(self.alloc_slot(
				self.0
					.device
					.create_sampler(sampler_create_info, None)
					.map_err(AshAllocationError::from)?,
			)?)
		}
	}
}

impl BufferTableAccess<'_, Ash> {
	/// Create a new buffer directly from an ash's [`BufferCreateInfo`] and the flattened members of GpuAllocator's
	/// [`AllocationCreateDesc`] to allow for maximum customizability.
	///
	/// # Safety
	/// Size must be sufficient to store `T`. If `T` is a slice, `len` must be its length, otherwise it must be 1.
	/// Returned buffer will be uninitialized.
	#[allow(clippy::too_many_arguments)]
	pub unsafe fn alloc_ash_unchecked<T: BufferContent + ?Sized>(
		&self,
		ash_create_info: &ash::vk::BufferCreateInfo,
		usage: BindlessBufferUsage,
		location: MemoryLocation,
		allocation_scheme: BindlessAllocationScheme,
		len: usize,
		name: &str,
		prev_access_type: BufferAccess,
	) -> Result<MutDesc<Ash, MutBuffer<T>>, BufferAllocationError<Ash>> {
		unsafe {
			let buffer = self
				.0
				.device
				.create_buffer(ash_create_info, None)
				.map_err(AshAllocationError::from)?;
			if let Some(debug_marker) = self.0.extensions.debug_utils.as_ref() {
				debug_marker
					.set_debug_utils_object_name(
						&DebugUtilsObjectNameInfoEXT::default()
							.object_handle(buffer)
							.object_name(&CString::new(name).unwrap()),
					)
					.map_err(AshAllocationError::from)?;
			}
			let requirements = self.0.device.get_buffer_memory_requirements(buffer);
			let memory_allocation = self
				.0
				.memory_allocator()
				.allocate(&AllocationCreateDesc {
					requirements,
					name,
					location,
					allocation_scheme: allocation_scheme.to_gpu_allocator_buffer(buffer),
					linear: true,
				})
				.map_err(AshAllocationError::from)?;
			self.0
				.device
				.bind_buffer_memory(buffer, memory_allocation.memory(), memory_allocation.offset())
				.map_err(AshAllocationError::from)?;
			Ok(self.alloc_slot(BufferSlot {
				platform: AshBuffer {
					buffer,
					allocation: AshMemoryAllocation::new(memory_allocation),
				},
				len,
				size: ash_create_info.size,
				usage,
				strong_refs: Default::default(),
				access_lock: AccessLock::new(prev_access_type),
				debug_name: name.to_string(),
			})?)
		}
	}
}
