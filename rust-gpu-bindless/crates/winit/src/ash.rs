use crate::event_loop::EventLoopExecutor;
use crate::window_ref::WindowRef;
use anyhow::Context;
use ash::ext::metal_surface;
use ash::khr::{android_surface, surface, wayland_surface, win32_surface, xcb_surface, xlib_surface};
use ash::prelude::VkResult;
use ash::vk::{
	ColorSpaceKHR, CompositeAlphaFlagsKHR, Fence, FenceCreateInfo, PipelineStageFlags, PresentInfoKHR, PresentModeKHR,
	Semaphore, SemaphoreCreateInfo, SharingMode, SubmitInfo, SurfaceTransformFlagsKHR, SwapchainCreateInfoKHR,
	TimelineSemaphoreSubmitInfo,
};
use rust_gpu_bindless_core::backing::table::RcTableSlot;
use rust_gpu_bindless_core::descriptor::{
	Bindless, BindlessImageCreateInfo, BindlessImageUsage, Extent, Format, Image2d, ImageAllocationError, ImageSlot,
	MutDesc, MutDescExt, MutImage, SampleCount, SwapchainImageId,
};
use rust_gpu_bindless_core::pipeline::{AccessLock, AccessLockError, ImageAccess};
use rust_gpu_bindless_core::platform::ash::{
	Ash, AshAllocationError, AshImage, AshMemoryAllocation, AshPendingExecution,
};
use std::ffi::CStr;
use std::fmt::Display;
use std::fmt::{Debug, Formatter};
use std::time::Duration;
use thiserror::Error;
use winit::event::{Event, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle};

pub fn ash_enumerate_required_extensions(display_handle: RawDisplayHandle) -> VkResult<&'static [&'static CStr]> {
	Ok(match display_handle {
		RawDisplayHandle::Windows(_) => &[surface::NAME, win32_surface::NAME],
		RawDisplayHandle::Wayland(_) => &[surface::NAME, wayland_surface::NAME],
		RawDisplayHandle::Xlib(_) => &[surface::NAME, xlib_surface::NAME],
		RawDisplayHandle::Xcb(_) => &[surface::NAME, xcb_surface::NAME],
		RawDisplayHandle::Android(_) => &[surface::NAME, android_surface::NAME],
		RawDisplayHandle::AppKit(_) | RawDisplayHandle::UiKit(_) => &[surface::NAME, metal_surface::NAME],
		_ => return Err(ash::vk::Result::ERROR_EXTENSION_NOT_PRESENT),
	})
}

#[derive(Debug, Copy, Clone)]
pub struct AshSwapchainParams {
	pub image_count: u32,
	pub format: Format,
	pub colorspace: ColorSpaceKHR,
	pub image_usage: BindlessImageUsage,
	pub present_mode: PresentModeKHR,
	pub pre_transform: SurfaceTransformFlagsKHR,
	pub composite_alpha: CompositeAlphaFlagsKHR,
}

#[derive(Debug, Copy, Clone)]
pub enum SwapchainImageFormatPreference {
	UNORM,
	SRGB,
}

impl SwapchainImageFormatPreference {
	pub fn value_format(&self, format: Format) -> i32 {
		match self {
			SwapchainImageFormatPreference::UNORM => match format {
				Format::R8G8B8A8_UNORM => 50,
				Format::B8G8R8A8_UNORM => 40,
				Format::R8G8B8A8_SRGB => 30,
				Format::B8G8R8A8_SRGB => 20,
				_ => 0,
			},
			SwapchainImageFormatPreference::SRGB => match format {
				Format::R8G8B8A8_SRGB => 50,
				Format::B8G8R8A8_SRGB => 40,
				Format::R8G8B8A8_UNORM => 30,
				Format::B8G8R8A8_UNORM => 20,
				_ => 0,
			},
		}
	}
}

impl AshSwapchainParams {
	fn create_info(&self, surface: ash::vk::SurfaceKHR, extent: Extent) -> SwapchainCreateInfoKHR<'_> {
		SwapchainCreateInfoKHR::default()
			.surface(surface)
			.min_image_count(self.image_count)
			.image_format(self.format)
			.image_color_space(self.colorspace)
			.image_extent(extent.into())
			.image_array_layers(1)
			.image_usage(self.image_usage.to_ash_image_usage_flags())
			.image_sharing_mode(SharingMode::EXCLUSIVE)
			.pre_transform(self.pre_transform)
			.composite_alpha(self.composite_alpha)
			.present_mode(self.present_mode)
			.clipped(true)
	}

	pub unsafe fn automatic_best(
		bindless: &Bindless<Ash>,
		surface: &ash::vk::SurfaceKHR,
		image_usage: BindlessImageUsage,
		format_preference: SwapchainImageFormatPreference,
	) -> anyhow::Result<Self> {
		unsafe {
			let surface_ext = bindless.extensions.surface();
			let phy = bindless.physical_device;
			let capabilities = surface_ext.get_physical_device_surface_capabilities(phy, *surface)?;

			let (format, colorspace) = surface_ext
				.get_physical_device_surface_formats(phy, *surface)?
				.into_iter()
				.map(|e| (e.format, e.color_space))
				.filter(|(_, c)| *c == ColorSpaceKHR::SRGB_NONLINEAR)
				.max_by_key(|(format, _)| format_preference.value_format(*format))
				.context("No SRGB_NONLINEAR surface format available")?;

			let present_mode = surface_ext
				.get_physical_device_surface_present_modes(phy, *surface)?
				.into_iter()
				.max_by_key(|p| match *p {
					PresentModeKHR::MAILBOX => 100,
					_ => 0,
				})
				// FIFO is always available
				.unwrap_or(PresentModeKHR::FIFO);

			let image_count = {
				let mut best_count = if present_mode == PresentModeKHR::MAILBOX {
					// try to request a 3 image swapchain if we use MailBox
					3
				} else {
					// Fifo wants 2 images
					2
				};
				if capabilities.max_image_count != 0 {
					best_count = best_count.min(capabilities.max_image_count);
				}
				best_count.max(capabilities.min_image_count)
			};

			Ok(Self {
				image_count,
				format,
				colorspace,
				image_usage: image_usage | BindlessImageUsage::SWAPCHAIN,
				present_mode,
				pre_transform: SurfaceTransformFlagsKHR::IDENTITY,
				composite_alpha: CompositeAlphaFlagsKHR::OPAQUE,
			})
		}
	}
}

/// A binary semaphore for swapchain operations
pub struct SwapchainSync {
	acquire: Semaphore,
	present: Semaphore,
	acquire_fence: Fence,
}

impl SwapchainSync {
	pub unsafe fn new(bindless: &Bindless<Ash>) -> anyhow::Result<Self> {
		unsafe {
			Ok(Self {
				acquire: bindless
					.device
					.create_semaphore(&SemaphoreCreateInfo::default(), None)?,
				present: bindless
					.device
					.create_semaphore(&SemaphoreCreateInfo::default(), None)?,
				acquire_fence: bindless.device.create_fence(&FenceCreateInfo::default(), None)?,
			})
		}
	}

	pub unsafe fn destroy(&mut self, bindless: &Bindless<Ash>) {
		unsafe {
			bindless.device.destroy_semaphore(self.acquire, None);
			bindless.device.destroy_semaphore(self.present, None);
			bindless.device.destroy_fence(self.acquire_fence, None);
		}
	}
}

pub struct AshSwapchain {
	bindless: Bindless<Ash>,
	event_loop: EventLoopExecutor,
	window: WindowRef,
	surface: ash::vk::SurfaceKHR,
	params: AshSwapchainParams,
	swapchain: ash::vk::SwapchainKHR,
	images: Vec<Option<RcTableSlot>>,
	image_semaphores: Vec<Option<SwapchainSync>>,
	sync_pool: Vec<SwapchainSync>,
	should_recreate: bool,
}

impl AshSwapchain {
	pub async unsafe fn new(
		bindless: &Bindless<Ash>,
		event_loop: &EventLoopExecutor,
		window_ref: WindowRef,
		params: impl FnOnce(&ash::vk::SurfaceKHR, &ActiveEventLoop) -> anyhow::Result<AshSwapchainParams> + Send + 'static,
	) -> anyhow::Result<Self> {
		unsafe {
			let bindless = bindless.clone();
			let event_loop_clone = event_loop.clone();
			event_loop
				.spawn(move |e| {
					let window = window_ref.get(e);
					let surface = ash_window::create_surface(
						&bindless.entry,
						&bindless.instance,
						window.display_handle()?.as_raw(),
						window.window_handle()?.as_raw(),
						None,
					)?;
					let params = params(&surface, e)?;
					let (swapchain, images) = Self::create_swapchain(
						&bindless,
						&window_ref,
						surface,
						&params,
						ash::vk::SwapchainKHR::null(),
						e,
					)?;
					let images = images.into_iter().map(Some).collect::<Vec<_>>();
					let image_semaphores = (0..images.len()).map(|_| None).collect();
					Ok(Self {
						bindless,
						event_loop: event_loop_clone,
						window: window_ref,
						params,
						surface,
						swapchain,
						images,
						image_semaphores,
						sync_pool: Vec::new(),
						should_recreate: false,
					})
				})
				.await
		}
	}

	unsafe fn create_swapchain(
		bindless: &Bindless<Ash>,
		window: &WindowRef,
		surface: ash::vk::SurfaceKHR,
		params: &AshSwapchainParams,
		old_swapchain: ash::vk::SwapchainKHR,
		e: &ActiveEventLoop,
	) -> anyhow::Result<(ash::vk::SwapchainKHR, Vec<RcTableSlot>)> {
		profiling::function_scope!();
		unsafe {
			let extent = {
				let window_size = window.get(e).inner_size();
				let surface_ext = bindless.extensions.surface();
				let capabilities =
					surface_ext.get_physical_device_surface_capabilities(bindless.physical_device, surface)?;
				let min = capabilities.min_image_extent;
				let max = capabilities.max_image_extent;
				Extent::from([
					u32::clamp(window_size.width, min.width, max.width),
					u32::clamp(window_size.height, min.height, max.height),
				])
			};

			let ext_swapchain = bindless.extensions.swapchain();
			let swapchain = ext_swapchain
				.create_swapchain(&params.create_info(surface, extent).old_swapchain(old_swapchain), None)
				.map_err(AshAllocationError::from)?;
			let images = ext_swapchain
				.get_swapchain_images(swapchain)
				.map_err(AshAllocationError::from)?
				.into_iter()
				.enumerate()
				.map(|(id, image)| Self::create_swapchain_image(bindless, params, extent, id as u32, image))
				.collect::<Result<Vec<_>, _>>()?;
			Ok((swapchain, images))
		}
	}

	unsafe fn create_swapchain_image(
		bindless: &Bindless<Ash>,
		params: &AshSwapchainParams,
		extent: Extent,
		id: u32,
		image: ash::vk::Image,
	) -> Result<RcTableSlot, ImageAllocationError<Ash>> {
		unsafe {
			let debug_name = format!("Swapchain Image {}", id);
			bindless
				.set_debug_object_name(image, &debug_name)
				.map_err(AshAllocationError::from)?;
			let image_view = bindless.create_image_view(
				image,
				&BindlessImageCreateInfo::<Image2d> {
					format: params.format,
					extent,
					mip_levels: 1,
					array_layers: 1,
					samples: SampleCount::default(),
					usage: params.image_usage,
					allocation_scheme: Default::default(),
					name: &debug_name,
					_phantom: Default::default(),
				},
			)?;
			let image = bindless.image().alloc_slot::<Image2d>(ImageSlot {
				platform: AshImage {
					image,
					image_view,
					allocation: AshMemoryAllocation::none(),
				},
				usage: params.image_usage,
				format: params.format,
				extent,
				mip_levels: 1,
				array_layers: 1,
				access_lock: AccessLock::new_locked(),
				debug_name,
				swapchain_image_id: SwapchainImageId::new(id),
			})?;
			Ok(image.into_inner().0)
		}
	}

	pub fn event_loop(&self) -> &EventLoopExecutor {
		&self.event_loop
	}

	pub fn window(&self) -> &WindowRef {
		&self.window
	}

	pub unsafe fn surface(&self) -> &ash::vk::SurfaceKHR {
		&self.surface
	}

	pub fn params(&self) -> &AshSwapchainParams {
		&self.params
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

	pub async fn acquire_image(
		&mut self,
		timeout: Option<Duration>,
	) -> anyhow::Result<MutDesc<Ash, MutImage<Image2d>>> {
		unsafe {
			profiling::scope!("acquire_image");
			let swapchain_ext = self.bindless.extensions.swapchain();
			const RECREATE_ATTEMPTS: u32 = 10;
			for _ in 0..RECREATE_ATTEMPTS {
				if self.should_recreate {
					self.should_recreate = false;

					self.bindless.device.device_wait_idle()?;
					let new = {
						let bindless = self.bindless.clone();
						let window = self.window.clone();
						let surface = self.surface;
						let params = self.params;
						let swapchain = self.swapchain;
						self.event_loop
							.spawn(move |e| Self::create_swapchain(&bindless, &window, surface, &params, swapchain, e))
							.await?
					};
					swapchain_ext.destroy_swapchain(self.swapchain, None);
					self.swapchain = new.0;
					assert_eq!(self.images.len(), new.1.len());
					for (i, image) in new.1.into_iter().enumerate() {
						drop(self.images[i].replace(image));
					}
				}

				let sync = self
					.sync_pool
					.pop()
					.map_or_else(|| SwapchainSync::new(&self.bindless), Ok)?;
				match swapchain_ext.acquire_next_image(
					self.swapchain,
					timeout.map(|a| a.as_nanos() as u64).unwrap_or(!0),
					sync.acquire,
					Fence::null(),
				) {
					Ok((id, suboptimal)) => {
						if suboptimal {
							self.should_recreate = true;
						}
						let image = self.images[id as usize].take().context("Image {i} was given out")?;
						let device = &self.bindless.device;
						let execution = self.bindless.execution_manager.new_execution_no_frame()?;
						{
							let queue = self.bindless.queue.lock();
							device.queue_submit(
								*queue,
								&[SubmitInfo::default()
									.wait_semaphores(&[sync.acquire])
									.wait_dst_stage_mask(&[PipelineStageFlags::ALL_COMMANDS])
									.signal_semaphores(&[execution.resource().semaphore])
									.push_next(
										&mut TimelineSemaphoreSubmitInfo::default()
											.wait_semaphore_values(&[0])
											.signal_semaphore_values(&[execution.resource().timeline_value]),
									)],
								sync.acquire_fence,
							)?;
						}
						let pending = AshPendingExecution::new(&execution);
						self.bindless.execution_manager.submit_for_waiting(execution)?;
						if let Some(e) = self.image_semaphores[id as usize].replace(sync) {
							device.wait_for_fences(&[e.acquire_fence], true, !0)?;
							device.reset_fences(&[e.acquire_fence])?;
							self.sync_pool.push(e);
						}
						let desc = MutDesc::<Ash, MutImage<Image2d>>::new(image, pending);
						desc.inner_slot().access_lock.unlock(ImageAccess::Present);
						return Ok(desc);
					}
					Err(ash::vk::Result::ERROR_OUT_OF_DATE_KHR) => {
						self.sync_pool.push(sync);
						self.should_recreate = true;
					}
					Err(e) => {
						self.sync_pool.push(sync);
						return Err(e.into());
					}
				}
			}
			panic!(
				"looped {} times trying to acquire swapchain image and failed repeatedly!",
				RECREATE_ATTEMPTS
			);
		}
	}

	pub fn present_image(&mut self, image: MutDesc<Ash, MutImage<Image2d>>) -> Result<(), PresentError> {
		unsafe {
			profiling::scope!("present_image");
			let id = {
				let slot = image.inner_slot();
				if !slot.usage.contains(BindlessImageUsage::SWAPCHAIN) {
					return Err(PresentError::NotASwapchainImage(slot.debug_name.clone()));
				}
				let id = slot
					.swapchain_image_id
					.get()
					.expect("Swapchain usage without swapchain_image_id set");
				if self.images[id as usize].is_some() {
					return Err(PresentError::SwapchainIdOccupied(id));
				}
				let access = slot.access_lock.try_lock()?;
				if !matches!(access, ImageAccess::Present | ImageAccess::General) {
					slot.access_lock.unlock(access);
					return Err(PresentError::IncorrectLayout(slot.debug_name.clone(), access));
				}
				id
			};

			let device = &self.bindless.device;
			let swapchain_ext = self.bindless.extensions.swapchain();
			let (rc_slot, last) = image.into_inner();
			let semaphore = self.image_semaphores[id as usize]
				.as_ref()
				.expect("missing image_semaphore {e:?} in slot {id}")
				.present;

			let dependency = last.upgrade_ash_resource();

			let suboptimal = {
				let queue = self.bindless.queue.lock();
				device.queue_submit(
					*queue,
					&[SubmitInfo::default()
						.wait_semaphores(dependency.as_ref().map(|a| a.resource().semaphore).as_slice())
						.wait_dst_stage_mask(dependency.as_ref().map(|_| PipelineStageFlags::ALL_COMMANDS).as_slice())
						.signal_semaphores(&[semaphore])
						.push_next(
							&mut TimelineSemaphoreSubmitInfo::default()
								.wait_semaphore_values(
									dependency.as_ref().map(|a| a.resource().timeline_value).as_slice(),
								)
								.signal_semaphore_values(&[0]),
						)],
					Fence::null(),
				)?;
				match swapchain_ext.queue_present(
					*queue,
					&PresentInfoKHR::default()
						.wait_semaphores(&[semaphore])
						.swapchains(&[self.swapchain])
						.image_indices(&[id]),
				) {
					Ok(e) => Ok(e),
					Err(ash::vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(true),
					Err(e) => Err(e),
				}?
			};
			if suboptimal {
				self.should_recreate = true;
			}

			self.images[id as usize].replace(rc_slot);
			Ok(())
		}
	}
}

#[derive(Error)]
pub enum PresentError {
	#[error("Vk Error: {0}")]
	Vk(#[from] ash::vk::Result),
	#[error("AccessLockError: {0}")]
	AccessLockError(#[from] AccessLockError),
	#[error("Image {0} must be a swapchain image of this swapchain to be presentable")]
	NotASwapchainImage(String),
	#[error("Image {0} must be in ImageAccess::Present or General, but is in {1:?} access")]
	IncorrectLayout(String, ImageAccess),
	#[error("Swapchain Image id {0} was already occupied by another image")]
	SwapchainIdOccupied(u32),
}

impl Debug for PresentError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(self, f)
	}
}

impl Drop for AshSwapchain {
	fn drop(&mut self) {
		unsafe {
			self.bindless.device.device_wait_idle().unwrap();
			for x in self.sync_pool.iter_mut() {
				x.destroy(&self.bindless);
			}
			let ext_swapchain = &self.bindless.extensions.swapchain();
			ext_swapchain.destroy_swapchain(self.swapchain, None);
			let surface_ext = self.bindless.extensions.surface();
			surface_ext.destroy_surface(self.surface, None);
		}
	}
}
