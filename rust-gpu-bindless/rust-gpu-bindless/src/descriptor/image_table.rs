use crate::backing::range_set::DescriptorIndexIterator;
use crate::backing::table::{DrainFlushQueue, RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::mutdesc::{MutDesc, MutDescExt};
use crate::descriptor::{Bindless, BindlessAllocationScheme, DescriptorCounts, Extent};
use crate::pipeline::access_lock::AccessLock;
use crate::pipeline::access_type::ImageAccess;
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::descriptor::{Image, ImageType, MutImage};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Arc, Weak};

impl<T: ImageType> DescContentCpu for Image<T> {
	type DescTable<P: BindlessPlatform> = ImageTable<P>;
	type VulkanType<P: BindlessPlatform> = ImageSlot<P>;
	type Slot<P: BindlessPlatform> = ImageSlot<P>;

	fn get_slot<P: BindlessPlatform>(slot: &RcTableSlot) -> &Self::Slot<P> {
		slot.try_deref::<ImageInterface<P>>().unwrap()
	}

	fn deref_table<P: BindlessPlatform>(slot: &Self::Slot<P>) -> &Self::VulkanType<P> {
		slot
	}
}

impl<T: ImageType> DescContentCpu for MutImage<T> {
	type DescTable<P: BindlessPlatform> = ImageTable<P>;
	type VulkanType<P: BindlessPlatform> = ImageSlot<P>;
	type Slot<P: BindlessPlatform> = ImageSlot<P>;

	fn get_slot<P: BindlessPlatform>(slot: &RcTableSlot) -> &Self::Slot<P> {
		slot.try_deref::<ImageInterface<P>>().unwrap()
	}

	fn deref_table<P: BindlessPlatform>(slot: &Self::Slot<P>) -> &Self::VulkanType<P> {
		slot
	}
}

impl<P: BindlessPlatform> DescTable for ImageTable<P> {}

pub struct ImageSlot<P: BindlessPlatform> {
	pub image: P::Image,
	pub imageview: P::ImageView,
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
	pub memory_allocation: P::MemoryAllocation,
}

pub struct ImageTable<P: BindlessPlatform> {
	table: Arc<Table<ImageInterface<P>>>,
}

impl<P: BindlessPlatform> ImageTable<P> {
	pub fn new(table_sync: &Arc<TableSync>, counts: DescriptorCounts, bindless: Weak<Bindless<P>>) -> Self {
		Self {
			table: table_sync.register(counts.image, ImageInterface { bindless }).unwrap(),
		}
	}
}

pub struct ImageTableAccess<'a, P: BindlessPlatform>(pub &'a Arc<Bindless<P>>);

impl<'a, P: BindlessPlatform> Deref for ImageTableAccess<'a, P> {
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

impl<'a, T: ImageType> Default for BindlessImageCreateInfo<'a, T> {
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

impl<'a, P: BindlessPlatform> ImageTableAccess<'a, P> {
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
	) -> Result<MutDesc<P, MutImage<T>>, P::AllocationError> {
		unsafe { Ok(MutDesc::new(self.table.alloc_slot(image)?)) }
	}

	pub(crate) fn flush_queue(&self) -> DrainFlushQueue<'_, ImageInterface<P>> {
		self.table.drain_flush_queue()
	}

	pub fn alloc<T: ImageType>(
		&self,
		create_info: &BindlessImageCreateInfo<T>,
	) -> Result<MutDesc<P, MutImage<T>>, P::AllocationError> {
		unsafe {
			let (image, imageview, memory_allocation) = self.0.platform.alloc_image(create_info)?;
			self.alloc_slot(ImageSlot {
				image,
				imageview,
				usage: create_info.usage,
				format: create_info.format,
				extent: create_info.extent,
				mip_levels: create_info.mip_levels,
				array_layers: create_info.array_layers,
				access_lock: AccessLock::new(create_info.usage.initial_image_access()),
				memory_allocation,
			})
		}
	}
}

pub struct ImageInterface<P: BindlessPlatform> {
	bindless: Weak<Bindless<P>>,
}

impl<P: BindlessPlatform> TableInterface for ImageInterface<P> {
	type Slot = ImageSlot<P>;

	fn drop_slots<'a>(&self, indices: impl DescriptorIndexIterator<'a, Self>) {
		unsafe {
			if let Some(bindless) = self.bindless.upgrade() {
				bindless
					.platform
					.destroy_images(&bindless.global_descriptor_set(), indices);
			}
		}
	}

	fn flush<'a>(&self, _flush_queue: impl DescriptorIndexIterator<'a, Self>) {
		// do nothing, flushing of descriptors is handled differently
	}
}
