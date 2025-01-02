#![cfg(test)]

use crate::debugger;
use pollster::block_on;
use rust_gpu_bindless::generic::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessImageCreateInfo,
	BindlessImageUsage, DescriptorCounts, Extent, Format, Image2d, MutDescBufferExt,
};
use rust_gpu_bindless::generic::pipeline::access_buffer::MutBufferAccessExt;
use rust_gpu_bindless::generic::pipeline::access_image::MutImageAccessExt;
use rust_gpu_bindless::generic::pipeline::access_type::{HostAccess, TransferRead, TransferWrite};
use rust_gpu_bindless::generic::platform::ash::{
	ash_init_single_graphics_queue, Ash, AshSingleGraphicsQueueCreateInfo,
};
use rust_gpu_bindless::generic::platform::BindlessPipelinePlatform;
use rust_gpu_bindless::spirv_std::glam::UVec2;
use std::sync::Arc;

#[test]
fn test_image_copy_ash() -> anyhow::Result<()> {
	unsafe {
		let bindless = Bindless::<Ash>::new(
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

async fn test_image_copy<P: BindlessPipelinePlatform>(bindless: &Arc<Bindless<P>>) -> anyhow::Result<()> {
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
		let mut staging_upload = staging_upload.access::<TransferRead>(cmd)?;
		let mut image = image.access::<TransferWrite>(cmd)?;
		let mut staging_download = staging_download.access::<TransferWrite>(cmd)?;

		cmd.copy_buffer_to_image(&mut staging_upload, &mut image)?;
		let mut image = image.transition::<TransferRead>()?;
		unsafe { cmd.copy_image_to_buffer(&mut image, &mut staging_download)? };

		Ok(staging_download.transition::<HostAccess>()?.into_desc())
	})?;

	// 5. downloads the data from `staging_download` and verifies that it hasn't corrupted
	let result = staging_download.mapped().await?.read_iter().collect::<Vec<_>>();
	assert_eq!(&*result, &*pixels);
	Ok(())
}
