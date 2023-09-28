use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use smallvec::SmallVec;
use vulkano::{swapchain, VulkanError};
use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::image::ImageUsage;
use vulkano::image::view::ImageView;
use vulkano::swapchain::{acquire_next_image, ColorSpace, CompositeAlpha, PresentMode, Surface, SwapchainAcquireFuture, SwapchainCreateInfo};
use vulkano::swapchain::ColorSpace::SrgbNonLinear;
use vulkano::swapchain::PresentMode::{Fifo, Mailbox};
use vulkano::sync::Sharing;

pub struct Swapchain {
	device: Arc<Device>,
	surface: Arc<Surface>,
	format: Format,
	colorspace: ColorSpace,
	present_mode: PresentMode,
	image_count: u32,
	image_usage: ImageUsage,
}

impl Swapchain {
	pub fn new(device: Arc<Device>, surface: Arc<Surface>, window_size: [u32; 2]) -> (Arc<Swapchain>, SwapchainController) {
		let surface_capabilities = device.physical_device().surface_capabilities(&surface, Default::default()).unwrap();
		let image_usage = surface_capabilities.supported_usage_flags;

		let format;
		let colorspace;
		{
			let formats: SmallVec<[_; 8]> = device.physical_device().surface_formats(&surface, Default::default()).unwrap()
				.into_iter().filter(|f| f.1 == SrgbNonLinear).collect();
			let f = *formats.iter().find(|f| f.0 == Format::B8G8R8A8_SRGB)
				.or_else(|| formats.iter().find(|f| f.0 == Format::R8G8B8A8_SRGB))
				.or_else(|| formats.iter().find(|f| f.0 == Format::B8G8R8A8_UNORM))
				.or_else(|| formats.iter().find(|f| f.0 == Format::R8G8B8A8_UNORM))
				.unwrap_or_else(|| &formats[0]);
			format = f.0;
			colorspace = f.1;
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
			image_count = surface_capabilities.min_image_count.min(best_count)
				.max(surface_capabilities.max_image_count.unwrap_or(best_count))
		}

		let swapchain = Arc::new(Swapchain {
			device,
			surface,
			format,
			colorspace,
			present_mode,
			image_count,
			image_usage,
		});
		(swapchain.clone(), SwapchainController::new(swapchain, window_size))
	}

	pub fn surface(&self) -> &Arc<Surface> {
		&self.surface
	}
	pub fn format(&self) -> Format {
		self.format
	}
	pub fn colorspace(&self) -> ColorSpace {
		self.colorspace
	}
	pub fn present_mode(&self) -> PresentMode {
		self.present_mode
	}
	pub fn image_count(&self) -> u32 {
		self.image_count
	}
	pub fn image_usage(&self) -> ImageUsage {
		self.image_usage
	}

	fn create_info(self, window_size: [u32; 2]) -> SwapchainCreateInfo {
		SwapchainCreateInfo {
			image_format: self.format,
			image_color_space: self.colorspace,
			present_mode: self.present_mode,
			min_image_count: self.image_count,
			image_usage: self.image_usage,
			image_extent: window_size,
			image_sharing: Sharing::Exclusive,
			composite_alpha: CompositeAlpha::Opaque,
			..Default::default()
		}
	}
}

pub struct SwapchainController {
	parent: Arc<Swapchain>,
	swapchain: Arc<swapchain::Swapchain>,
	images: SmallVec<[Arc<ImageView>; 4]>,
	should_recreate: bool,
}

impl SwapchainController {
	fn new(p: Arc<Swapchain>, window_size: [u32; 2]) -> Self {
		let (swapchain, images) = swapchain::Swapchain::new(p.device.clone(), p.surface.clone(), p.create_info(window_size)).unwrap();
		Self {
			parent: p,
			swapchain,
			images: images.into_iter().map(|i| ImageView::new_default(i).unwrap()).collect(),
			should_recreate: false,
		}
	}

	pub fn acquire_image(&mut self, window_size: [u32; 2], timeout: Option<Duration>) -> (SwapchainAcquireFuture, &Arc<ImageView>) {
		const RECREATE_ATTEMPTS: u32 = 10;
		for _ in 0..RECREATE_ATTEMPTS {
			if self.should_recreate {
				self.should_recreate = false;

				let new = self.swapchain.recreate(self.parent.create_info(window_size)).unwrap();
				self.swapchain = new.0;
				self.images = new.1.into_iter().map(|i| ImageView::new_default(i).unwrap()).collect();
			}

			match acquire_next_image(self.swapchain.clone(), timeout) {
				Ok((_, suboptimal, future)) => {
					if suboptimal {
						// suboptimal recreates swapchain next frame, recreating without presenting this frame is bugged in vulkano
						// https://github.com/vulkano-rs/vulkano/issues/2229
						self.should_recreate = true;
					}
					let image = &self.images[future.image_index() as usize];
					return (future, image);
				}
				Err(e) => {
					match e.unwrap() {
						VulkanError::OutOfDate => {
							// failed to acquire images, recreate swapchain this frame
							self.should_recreate = true;
						}
						e => panic!("{}", e),
					}
				}
			}
		}
		panic!("looped {} times trying to acquire swapchain image and failed repeatedly!", RECREATE_ATTEMPTS);
	}
}

impl Deref for SwapchainController {
	type Target = Arc<Swapchain>;

	fn deref(&self) -> &Self::Target {
		&self.parent
	}
}
