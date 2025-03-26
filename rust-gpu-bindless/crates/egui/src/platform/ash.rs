use crate::platform::EguiBindlessPlatform;
use rust_gpu_bindless_core::platform::ash::Ash;

unsafe impl EguiBindlessPlatform for Ash {
	unsafe fn max_image_dimensions_2d(&self) -> u32 {
		unsafe {
			self.instance
				.get_physical_device_properties(self.physical_device)
				.limits
				.max_image_dimension2_d
		}
	}
}
