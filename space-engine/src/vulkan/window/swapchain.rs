use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::thread::yield_now;
use std::time::Duration;

use parking_lot::Mutex;
use vulkano::device::{Device, DeviceOwned};
use vulkano::format::Format;
use vulkano::image::{ImageUsage, SwapchainImage};
use vulkano::swapchain;
use vulkano::swapchain::{acquire_next_image, CompositeAlpha, Surface, SwapchainAcquireFuture, SwapchainCreateInfo, SwapchainCreationError};
use vulkano::swapchain::ColorSpace::SrgbNonLinear;
use vulkano::swapchain::PresentMode::{Fifo, Mailbox};
use vulkano::sync::Sharing;

use crate::reinit::{ReinitRef, Restart};

pub struct SwapchainState {
	old: Mutex<Weak<swapchain::Swapchain>>,
}

impl Default for SwapchainState {
	fn default() -> Self {
		Self { old: Mutex::new(Weak::new()) }
	}
}

pub struct Swapchain {
	swapchain: Arc<swapchain::Swapchain>,
	images: Vec<Arc<SwapchainImage>>,
	restart: Restart<Self>,
}

impl Swapchain {
	pub fn new(device: ReinitRef<Arc<Device>>, window_size: [u32; 2], surface: ReinitRef<Arc<Surface>>, state: ReinitRef<SwapchainState>, restart: Restart<Self>) -> Self {
		let mut old = state.old.lock();
		let (swapchain, images) = (|| {
			loop {
				let surface_capabilities = device.physical_device().surface_capabilities(&surface, Default::default()).unwrap();

				let format;
				{
					let formats: Vec<_> = device.physical_device().surface_formats(&surface, Default::default()).unwrap()
						.into_iter().filter(|f| f.1 == SrgbNonLinear).collect();
					format = *formats.iter().find(|f| f.0 == Format::B8G8R8A8_SRGB)
						.or_else(|| formats.iter().find(|f| f.0 == Format::R8G8B8A8_SRGB))
						.or_else(|| formats.iter().find(|f| f.0 == Format::B8G8R8A8_UNORM))
						.or_else(|| formats.iter().find(|f| f.0 == Format::R8G8B8A8_UNORM))
						.unwrap_or_else(|| &formats[0]);
				}

				let present_mode;
				{
					let present_modes = || device.physical_device().surface_present_modes(&surface).unwrap();
					present_mode = present_modes().find(|p| *p == Mailbox).unwrap_or(Fifo);
				}

				let image_count;
				{
					let best_count = if present_mode == Mailbox {
						// try to request a 3 image swapchain if we use MailBox
						3
					} else {
						// Fifo just uses min_image_count
						surface_capabilities.min_image_count
					};
					image_count = surface_capabilities.min_image_count.min(best_count).max(surface_capabilities.max_image_count.unwrap_or(best_count))
				}

				let info = SwapchainCreateInfo {
					min_image_count: image_count,
					image_format: Some(format.0),
					image_color_space: format.1,
					image_extent: window_size,
					image_usage: ImageUsage {
						color_attachment: true,
						..Default::default()
					},
					image_sharing: Sharing::Exclusive,
					composite_alpha: CompositeAlpha::Opaque,
					present_mode,
					..Default::default()
				};

				if let Some(old) = old.upgrade() {
					if *old.device() == *device && *old.surface() == *surface {
						return swapchain::Swapchain::recreate(
							&old,
							info,
						).unwrap();
					}
				}

				match swapchain::Swapchain::new(
					device.deref().clone(),
					surface.deref().clone(),
					info.clone(),
				) {
					Ok(e) => return e,
					Err(e) => {
						match e {
							SwapchainCreationError::SurfaceInUse => {
								// race condition between swapchain dropping and this: retry until it works
								yield_now();
							}
							SwapchainCreationError::ImageExtentNotSupported { .. } => {
								// may happen while user resizes window
							}
							_ => {
								panic!("{}", e);
							}
						}
					}
				}
			}
		})();
		*old = Arc::downgrade(&swapchain);

		Self { swapchain, images, restart }
	}
}

#[derive(Copy, Clone, Debug)]
pub enum AcquireError {
	Timeout,
	Restart,
}

impl Swapchain {
	pub fn acquire_image(&self, timeout: Option<Duration>) -> Result<SwapchainAcquireFuture, AcquireError> {
		match acquire_next_image(self.swapchain.clone(), timeout) {
			Ok(e) => {
				if e.1 {
					self.restart.restart();
					Err(AcquireError::Restart)
				} else {
					Ok(e.2)
				}
			}
			Err(e) => {
				match e {
					swapchain::AcquireError::Timeout => Err(AcquireError::Timeout),
					swapchain::AcquireError::SurfaceLost | swapchain::AcquireError::OutOfDate => {
						self.restart.restart();
						Err(AcquireError::Restart)
					}
					e => {
						panic!("{:?}", e);
					}
				}
			}
		}
	}

	pub fn images(&self) -> impl Iterator<Item=&Arc<SwapchainImage>> {
		self.images.iter()
	}
}

impl Deref for Swapchain {
	type Target = Arc<swapchain::Swapchain>;

	fn deref(&self) -> &Self::Target {
		&self.swapchain
	}
}
