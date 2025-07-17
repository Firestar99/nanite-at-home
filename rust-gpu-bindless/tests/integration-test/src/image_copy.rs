#![cfg(test)]

use crate::debugger;
use glam::UVec2;
use pollster::block_on;
use rust_gpu_bindless_core::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessImageCreateInfo,
	BindlessImageUsage, BindlessInstance, DescriptorCounts, Extent, Format, Image2d, MutDescBufferExt,
};
use rust_gpu_bindless_core::pipeline::{
	HostAccess, MutBufferAccessExt, MutImageAccessExt, TransferRead, TransferWrite,
};
use rust_gpu_bindless_core::platform::BindlessPipelinePlatform;
use rust_gpu_bindless_core::platform::ash::{Ash, AshSingleGraphicsQueueCreateInfo, ash_init_single_graphics_queue};

#[test]
fn test_image_copy_ash() -> anyhow::Result<()> {
	unsafe {
		let bindless = BindlessInstance::<Ash>::new(
			ash_init_single_graphics_queue(AshSingleGraphicsQueueCreateInfo {
				debug: debugger(),
				..AshSingleGraphicsQueueCreateInfo::default()
			})?,
			DescriptorCounts::REASONABLE_DEFAULTS,
		);
		block_on(test_image_copy(&bindless))?;
		Ok(())
	}
}

async fn test_image_copy<P: BindlessPipelinePlatform>(bindless: &Bindless<P>) -> anyhow::Result<()> {
	let extent = UVec2::new(32, 32);
	let format = Format::R8G8B8A8_UNORM;
	let len = (extent.x * extent.y * 4) as usize;
	let pixels = (0..len).map(|i| i as u8).collect::<Vec<_>>();

	let buffer_ci = |name: &'static str| BindlessBufferCreateInfo {
		name,
		usage: BindlessBufferUsage::MAP_WRITE
			| BindlessBufferUsage::MAP_READ
			| BindlessBufferUsage::TRANSFER_SRC
			| BindlessBufferUsage::TRANSFER_DST,
		allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
	};

	let staging_upload = bindless
		.buffer()
		.alloc_from_iter(&buffer_ci("staging_upload"), pixels.iter().copied())?;
	let image = bindless.image().alloc::<Image2d>(&BindlessImageCreateInfo {
		format,
		extent: Extent::from(extent),
		usage: BindlessImageUsage::TRANSFER_SRC | BindlessImageUsage::TRANSFER_DST,
		..BindlessImageCreateInfo::default()
	})?;
	let staging_download = bindless.buffer().alloc_slice::<u8>(&buffer_ci("staging_upload"), len)?;

	let staging_download = bindless.execute(|cmd| {
		let staging_upload = staging_upload.access::<TransferRead>(cmd)?;
		let image = image.access::<TransferWrite>(cmd)?;
		let staging_download = staging_download.access::<TransferWrite>(cmd)?;

		cmd.copy_buffer_to_image(&staging_upload, &image)?;
		let image = image.transition::<TransferRead>()?;
		unsafe { cmd.copy_image_to_buffer(&image, &staging_download)? };

		Ok(staging_download.transition::<HostAccess>()?.into_desc())
	})?;

	// 5. downloads the data from `staging_download` and verifies that it hasn't corrupted
	let result = staging_download.mapped().await?.read_iter().collect::<Vec<_>>();
	assert_eq!(&*result, &*pixels);
	Ok(())
}
