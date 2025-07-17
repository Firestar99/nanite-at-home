use ash::prelude::VkResult;
use ash::vk::{CommandBuffer, CommandBufferAllocateInfo};
use std::mem::MaybeUninit;

pub trait DeviceExt {
	unsafe fn allocate_command_buffer(&self, allocate_info: &CommandBufferAllocateInfo<'_>) -> VkResult<CommandBuffer>;
}

impl DeviceExt for ash::Device {
	unsafe fn allocate_command_buffer(&self, allocate_info: &CommandBufferAllocateInfo<'_>) -> VkResult<CommandBuffer> {
		unsafe {
			assert_eq!(allocate_info.command_buffer_count, 1);
			let mut buffer = MaybeUninit::uninit();
			(self.fp_v1_0().allocate_command_buffers)(self.handle(), allocate_info, buffer.as_mut_ptr())
				.assume_init_on_success(buffer)
		}
	}
}
