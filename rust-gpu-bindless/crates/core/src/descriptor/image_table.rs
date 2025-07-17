use crate::backing::range_set::DescriptorIndexIterator;
use crate::backing::table::{DrainFlushQueue, RcTableSlot, SlotAllocationError, Table, TableInterface, TableSync};
use crate::descriptor::{
	Bindless, BindlessAllocationScheme, DescContentCpu, DescTable, DescriptorCounts, Extent, MutDesc, MutDescExt,
	RCDesc, RCDescExt, WeakBindless,
};
use crate::pipeline::{AccessLock, ImageAccess};
use crate::platform::{BindlessPlatform, PendingExecution};
use rust_gpu_bindless_shaders::descriptor::{Image, ImageType, MutImage};
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use thiserror::Error;

impl<T: ImageType> DescContentCpu for Image<T> {
	type DescTable<P: BindlessPlatform> = ImageTable<P>;
}

impl<T: ImageType> DescContentCpu for MutImage<T> {
	type DescTable<P: BindlessPlatform> = ImageTable<P>;
}

impl<P: BindlessPlatform> DescTable<P> for ImageTable<P> {
	type Slot = ImageSlot<P>;

	fn get_slot(slot: &RcTableSlot) -> &Self::Slot {
		slot.try_deref::<ImageInterface<P>>().unwrap()
	}
}

pub struct SwapchainImageId(u32);

impl SwapchainImageId {
	pub fn new(id: u32) -> Self {
		Self(id)
	}

	pub fn get(&self) -> Option<u32> {
		if self.0 != !0 { Some(self.0) } else { None }
	}
}

impl Default for SwapchainImageId {
	fn default() -> Self {
		Self(!0)
	}
}

pub struct ImageSlot<P: BindlessPlatform> {
	pub platform: P::Image,
	pub usage: BindlessImageUsage,
	/// The image format
	pub format: Format,
	/// The extent of the image: width, height, depth. Only those relevant for the image's dimensionality are read, the
	/// other ignored.
	pub extent: Extent,
	/// The amount of mip levels.
	pub mip_levels: u32,
	/// The amount of array layers. Must be `1` if the image is not arrayed.
	pub array_layers: u32,
	pub access_lock: AccessLock<ImageAccess>,
	/// This may be replaced with a platform-specific getter, once you can query the name from gpu-allocator to not
	/// unnecessarily duplicate the String (see my PR https://github.com/Traverse-Research/gpu-allocator/pull/257)
	pub debug_name: String,
	pub swapchain_image_id: SwapchainImageId,
}

impl<P: BindlessPlatform> Deref for ImageSlot<P> {
	type Target = P::Image;

	fn deref(&self) -> &Self::Target {
		&self.platform
	}
}

impl<P: BindlessPlatform> ImageSlot<P> {
	pub fn debug_name(&self) -> &str {
		&self.debug_name
	}
}

pub trait ImageDescExt {
	fn extent(&self) -> Extent;
	fn format(&self) -> Format;
}

impl<P: BindlessPlatform, T: ImageType> ImageDescExt for RCDesc<P, Image<T>> {
	fn extent(&self) -> Extent {
		self.inner_slot().extent
	}

	fn format(&self) -> Format {
		self.inner_slot().format
	}
}

impl<P: BindlessPlatform, T: ImageType> ImageDescExt for MutDesc<P, MutImage<T>> {
	fn extent(&self) -> Extent {
		self.inner_slot().extent
	}

	fn format(&self) -> Format {
		self.inner_slot().format
	}
}

pub struct ImageTable<P: BindlessPlatform> {
	table: Arc<Table<ImageInterface<P>>>,
}

impl<P: BindlessPlatform> ImageTable<P> {
	pub fn new(table_sync: &Arc<TableSync>, counts: DescriptorCounts, bindless: WeakBindless<P>) -> Self {
		Self {
			table: table_sync.register(counts.image, ImageInterface { bindless }).unwrap(),
		}
	}
}

pub struct ImageTableAccess<'a, P: BindlessPlatform>(pub &'a Bindless<P>);

impl<P: BindlessPlatform> Deref for ImageTableAccess<'_, P> {
	type Target = ImageTable<P>;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0.image
	}
}

bitflags::bitflags! {
	/// Image usage specify how you may use the image. Missing flags are only validated during runtime.
	#[repr(transparent)]
	#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
	pub struct BindlessImageUsage: u64 {
		/// Can be used as a source of transfer operations
		const TRANSFER_SRC = 0b1;
		/// Can be used as a destination of transfer operations
		const TRANSFER_DST = 0b10;
		/// Can be sampled from with a sampler
		const SAMPLED = 0b100;
		/// Can be used as storage image
		const STORAGE = 0b1000;
		/// Can be used as framebuffer color attachment
		const COLOR_ATTACHMENT = 0b1_0000;
		/// Can be used as framebuffer depth/stencil attachment
		const DEPTH_STENCIL_ATTACHMENT = 0b10_0000;
		/// Image is part of a swapchain and may be used for presenting. You may not create an image with this usage
		/// yourself, and must acquire it from a swapchain.
		const SWAPCHAIN = 0b100_0000;
	}
}

impl BindlessImageUsage {
	#[inline]
	pub fn initial_image_access(&self) -> ImageAccess {
		ImageAccess::Undefined
	}
}

pub type Format = ash::vk::Format;

/// The amount of samples of the Image. Must be [`SampleCount::Sample1`] if the image is not multisampled.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum SampleCount {
	#[default]
	Sample1,
	Sample2,
	Sample4,
	Sample8,
	Sample16,
	Sample32,
	Sample64,
}

#[derive(Copy, Clone, Debug)]
pub struct BindlessImageCreateInfo<'a, T: ImageType> {
	/// The image format
	pub format: Format,
	/// The extent of the image: width, height, depth. Only those relevant for the image's dimensionality are read, the
	/// other ignored.
	pub extent: Extent,
	/// The amount of mip levels.
	pub mip_levels: u32,
	/// The amount of array layers. Must be `1` if the image is not arrayed.
	pub array_layers: u32,
	/// The amount of samples of the Image. Must be [`SampleCount::Sample1`] if the image is not multisampled.
	pub samples: SampleCount,
	/// Image usage specify how you may use the image. Missing flags are only validated during runtime.
	pub usage: BindlessImageUsage,
	/// Determines how this allocation should be managed.
	pub allocation_scheme: BindlessAllocationScheme,
	/// Name of the image, for tracking and debugging purposes
	pub name: &'a str,
	pub _phantom: PhantomData<T>,
}

impl<T: ImageType> Default for BindlessImageCreateInfo<'_, T> {
	fn default() -> Self {
		Self {
			format: Default::default(),
			extent: Extent::default(),
			mip_levels: 1,
			array_layers: 1,
			samples: SampleCount::default(),
			usage: BindlessImageUsage::default(),
			allocation_scheme: BindlessAllocationScheme::default(),
			name: "",
			_phantom: PhantomData,
		}
	}
}

impl<T: ImageType> BindlessImageCreateInfo<'_, T> {
	#[inline]
	pub fn validate<P: BindlessPlatform>(&self) -> Result<(), ImageAllocationError<P>> {
		if self.usage.contains(BindlessImageUsage::SWAPCHAIN) {
			Err(ImageAllocationError::SwapchainUsage {
				name: self.name.to_owned(),
			})
		} else {
			Ok(())
		}
	}
}

#[derive(Error)]
pub enum ImageAllocationError<P: BindlessPlatform> {
	#[error("Platform Error: {0}")]
	Platform(#[source] P::AllocationError),
	#[error("Slot Allocation Error: {0}")]
	Slot(#[from] SlotAllocationError),
	#[error("Image {name} must have at least one usage must be declared")]
	NoUsageDeclared { name: String },
	#[error("Image {name} must not be created with {swapchain:?}, instead swapchain images must be acquired from a swapchain", swapchain = BindlessImageUsage::SWAPCHAIN
	)]
	SwapchainUsage { name: String },
}

impl<P: BindlessPlatform> Debug for ImageAllocationError<P> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self, f)
	}
}

impl<P: BindlessPlatform> ImageTableAccess<'_, P> {
	/// Allocates a new slot for this image and imageview
	///
	/// # Safety
	/// Image's device and ImageView's device must be the same as the bindless device. Ownership of the Image and
	/// ImageView is transferred to this table. You may not access or drop it afterward, except by going though the
	/// returned `BoxDesc`. The generic T: ImageType must match the type of Image and ImageView.
	#[inline]
	pub unsafe fn alloc_slot<T: ImageType>(
		&self,
		image: ImageSlot<P>,
	) -> Result<MutDesc<P, MutImage<T>>, SlotAllocationError> {
		unsafe {
			Ok(MutDesc::new(
				self.table.alloc_slot(image)?,
				PendingExecution::<P>::new_completed(),
			))
		}
	}

	pub(crate) fn flush_queue(&self) -> DrainFlushQueue<'_, ImageInterface<P>> {
		self.table.drain_flush_queue()
	}

	pub fn alloc<T: ImageType>(
		&self,
		create_info: &BindlessImageCreateInfo<T>,
	) -> Result<MutDesc<P, MutImage<T>>, ImageAllocationError<P>> {
		unsafe {
			create_info.validate()?;
			let image = self
				.0
				.platform
				.alloc_image(create_info)
				.map_err(Into::<ImageAllocationError<P>>::into)?;
			Ok(self.alloc_slot(ImageSlot {
				platform: image,
				usage: create_info.usage,
				format: create_info.format,
				extent: create_info.extent,
				mip_levels: create_info.mip_levels,
				array_layers: create_info.array_layers,
				access_lock: AccessLock::new(create_info.usage.initial_image_access()),
				debug_name: create_info.name.to_string(),
				swapchain_image_id: SwapchainImageId::default(),
			})?)
		}
	}
}

pub struct ImageInterface<P: BindlessPlatform> {
	bindless: WeakBindless<P>,
}

impl<P: BindlessPlatform> TableInterface for ImageInterface<P> {
	type Slot = ImageSlot<P>;

	fn drop_slots<'a>(&self, indices: impl DescriptorIndexIterator<'a, Self>) {
		unsafe {
			if let Some(bindless) = self.bindless.upgrade() {
				bindless
					.platform
					.destroy_images(bindless.global_descriptor_set(), indices);
			}
		}
	}

	fn flush<'a>(&self, _flush_queue: impl DescriptorIndexIterator<'a, Self>) {
		// do nothing, flushing of descriptors is handled differently
	}
}
