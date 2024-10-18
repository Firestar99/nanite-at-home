use crate::window::event_loop::EventLoopExecutor;
use crate::window::window_ref::WindowRef;
use rust_gpu_bindless::frame_manager::Frame;
use smallvec::SmallVec;
use static_assertions::{assert_impl_all, assert_not_impl_all};
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use vulkano::device::{Device, DeviceOwned, Queue};
use vulkano::format::{Format, FormatFeatures};
use vulkano::image::view::ImageView;
use vulkano::image::ImageUsage;
use vulkano::swapchain::ColorSpace::SrgbNonLinear;
use vulkano::swapchain::PresentMode::{Fifo, Mailbox};
use vulkano::swapchain::{
	acquire_next_image, ColorSpace, CompositeAlpha, PresentMode, Surface, SurfaceInfo, SwapchainAcquireFuture,
	SwapchainCreateInfo, SwapchainPresentInfo,
};
use vulkano::sync::future::FenceSignalFuture;
use vulkano::sync::{GpuFuture, Sharing};
use vulkano::{swapchain, VulkanError};
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;

pub struct Swapchain {
	queue: Arc<Queue>,
	window: WindowRef,
	surface: Arc<Surface>,
	format: Format,
	colorspace: ColorSpace,
	present_mode: PresentMode,
	image_count: u32,
	image_usage: ImageUsage,
}

assert_impl_all!(Swapchain: Send, Sync);

impl Swapchain {
	pub async fn new(
		queue: Arc<Queue>,
		event_loop: EventLoopExecutor,
		window: WindowRef,
	) -> (Arc<Swapchain>, SwapchainController) {
		let (swapchain, vulkano_swapchain, images) = event_loop
			.spawn(move |event_loop| {
				let device = queue.device();
				let surface =
					Surface::from_window(device.instance().clone(), window.get_arc(event_loop).clone()).unwrap();
				let surface_capabilities = device
					.physical_device()
					.surface_capabilities(&surface, Default::default())
					.unwrap();

				let format;
				let colorspace;
				{
					let formats: SmallVec<[_; 8]> = device
						.physical_device()
						.surface_formats(&surface, Default::default())
						.unwrap()
						.into_iter()
						.filter(|f| f.1 == SrgbNonLinear)
						.collect();
					let f = *formats
						.iter()
						.find(|f| f.0 == Format::B8G8R8A8_UNORM)
						.or_else(|| formats.iter().find(|f| f.0 == Format::R8G8B8A8_UNORM))
						.unwrap_or_else(|| &formats[0]);
					format = f.0;
					colorspace = f.1;
				}

				let present_mode = device
					.physical_device()
					.surface_present_modes(&surface, SurfaceInfo::default())
					.unwrap()
					.into_iter()
					.find(|p| *p == Mailbox)
					.unwrap_or(Fifo);

				let image_count;
				{
					let best_count = if present_mode == Mailbox {
						// try to request a 3 image swapchain if we use MailBox
						3
					} else {
						// Fifo wants 2 images
						2
					};
					image_count = best_count
						.min(surface_capabilities.max_image_count.unwrap_or(best_count))
						.max(surface_capabilities.min_image_count)
				}

				let image_usage;
				{
					let features = device
						.physical_device()
						.format_properties(format)
						.unwrap()
						.optimal_tiling_features;
					let mut format_usage = ImageUsage::default();
					if features.contains(FormatFeatures::TRANSFER_SRC) {
						format_usage |= ImageUsage::TRANSFER_SRC;
					}
					if features.contains(FormatFeatures::TRANSFER_DST) {
						format_usage |= ImageUsage::TRANSFER_DST;
					}
					if features.contains(FormatFeatures::SAMPLED_IMAGE) {
						format_usage |= ImageUsage::SAMPLED;
					}
					if features.contains(FormatFeatures::STORAGE_IMAGE) {
						format_usage |= ImageUsage::STORAGE;
					}
					if features.contains(FormatFeatures::COLOR_ATTACHMENT) {
						format_usage |= ImageUsage::COLOR_ATTACHMENT;
					}
					if features.contains(FormatFeatures::DEPTH_STENCIL_ATTACHMENT) {
						format_usage |= ImageUsage::DEPTH_STENCIL_ATTACHMENT;
					}
					// fixme no idea what feature it must support
					// if features.contains(FormatFeatures::ATTACH) {
					// 	format_usage |= ImageUsage::INPUT_ATTACHMENT;
					// }
					image_usage = surface_capabilities.supported_usage_flags & format_usage;
				}

				let swapchain = Arc::new(Swapchain {
					queue,
					window,
					surface,
					format,
					colorspace,
					present_mode,
					image_count,
					image_usage,
				});
				let (vulkano_swapchain, images) = swapchain::Swapchain::new(
					swapchain.device().clone(),
					swapchain.surface.clone(),
					swapchain.create_info(event_loop),
				)
				.unwrap();
				let images = images.into_iter().map(|i| ImageView::new_default(i).unwrap()).collect();
				(swapchain, vulkano_swapchain, images)
			})
			.await;

		// needs to be constructed outside of EventLoopExecutor::spawn() to be able to move the event_loop
		let controller = SwapchainController {
			parent: swapchain.clone(),
			event_loop,
			vulkano_swapchain,
			images,
			should_recreate: false,
		};
		(swapchain, controller)
	}

	#[inline]
	pub fn queue(&self) -> &Arc<Queue> {
		&self.queue
	}
	#[inline]
	pub fn device(&self) -> &Arc<Device> {
		self.queue.device()
	}
	#[inline]
	pub fn surface(&self) -> &Arc<Surface> {
		&self.surface
	}
	#[inline]
	pub fn format(&self) -> Format {
		self.format
	}
	#[inline]
	pub fn colorspace(&self) -> ColorSpace {
		self.colorspace
	}
	#[inline]
	pub fn present_mode(&self) -> PresentMode {
		self.present_mode
	}
	#[inline]
	pub fn image_count(&self) -> u32 {
		self.image_count
	}
	#[inline]
	pub fn image_usage(&self) -> ImageUsage {
		self.image_usage
	}

	fn create_info(&self, event_loop: &EventLoopWindowTarget<()>) -> SwapchainCreateInfo {
		SwapchainCreateInfo {
			image_format: self.format,
			image_color_space: self.colorspace,
			present_mode: self.present_mode,
			min_image_count: self.image_count,
			image_usage: self.image_usage,
			image_extent: self.window.get(event_loop).inner_size().into(),
			image_sharing: Sharing::Exclusive,
			composite_alpha: CompositeAlpha::Opaque,
			..Default::default()
		}
	}
}

pub struct SwapchainController {
	parent: Arc<Swapchain>,
	event_loop: EventLoopExecutor,
	vulkano_swapchain: Arc<swapchain::Swapchain>,
	images: SmallVec<[Arc<ImageView>; 4]>,
	should_recreate: bool,
}

assert_impl_all!(SwapchainController: Send);
assert_not_impl_all!(SwapchainController: Sync);

impl SwapchainController {
	#[profiling::function]
	pub async fn acquire_image(&mut self, timeout: Option<Duration>) -> (SwapchainAcquireFuture, AcquiredImage) {
		const RECREATE_ATTEMPTS: u32 = 10;
		for _ in 0..RECREATE_ATTEMPTS {
			if self.should_recreate {
				self.should_recreate = false;

				let vulkano_swapchain = self.vulkano_swapchain.clone();
				let parent = self.parent.clone();
				let new = self
					.event_loop
					.spawn(move |event_loop| vulkano_swapchain.recreate(parent.create_info(event_loop)).unwrap())
					.await;
				self.vulkano_swapchain = new.0;
				self.images = new.1.into_iter().map(|i| ImageView::new_default(i).unwrap()).collect();
			}

			match acquire_next_image(self.vulkano_swapchain.clone(), timeout) {
				Ok((_, suboptimal, future)) => {
					if suboptimal {
						// suboptimal recreates swapchain next frame, recreating without presenting this frame is bugged in vulkano
						// and I think also not allowed in device, except via an extension?
						// https://github.com/vulkano-rs/vulkano/issues/2229
						self.should_recreate = true;
					}
					let acquired_image = AcquiredImage {
						controller: self,
						image_index: future.image_index(),
					};
					return (future, acquired_image);
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
		panic!(
			"looped {} times trying to acquire swapchain image and failed repeatedly!",
			RECREATE_ATTEMPTS
		);
	}

	pub fn force_recreate(&mut self) {
		self.should_recreate = true;
	}

	pub fn handle_input(&mut self, event: &Event<()>) {
		if let Event::WindowEvent {
			event: WindowEvent::Resized(_),
			..
		} = event
		{
			self.should_recreate = true;
		}
	}
}

impl Deref for SwapchainController {
	type Target = Arc<Swapchain>;

	fn deref(&self) -> &Self::Target {
		&self.parent
	}
}

/// Opinionated Design: does NOT deref to [`SwapchainController`] to make sure a new image isn't acquired before the previous one is presented
pub struct AcquiredImage<'a> {
	controller: &'a mut SwapchainController,
	image_index: u32,
}

impl<'a> AcquiredImage<'a> {
	pub fn image_view(&self) -> &Arc<ImageView> {
		&self.controller.images[self.image_index as usize]
	}

	#[profiling::function]
	pub fn present(
		self,
		frame: &Frame,
		future: impl GpuFuture + 'static,
	) -> Option<FenceSignalFuture<Box<dyn GpuFuture>>> {
		match frame.then_signal_fence_and_flush(
			future
				.then_swapchain_present(
					self.controller.queue.clone(),
					SwapchainPresentInfo::swapchain_image_index(
						self.controller.vulkano_swapchain.clone(),
						self.image_index,
					),
				)
				.boxed(),
		) {
			Ok(e) => Some(e),
			Err(e) => {
				match e.unwrap() {
					VulkanError::OutOfDate => {
						// failed to present image, recreate swapchain this frame
						self.controller.should_recreate = true;
						None
					}
					e => panic!("{}", e),
				}
			}
		}
	}
}
