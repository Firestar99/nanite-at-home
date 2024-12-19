#![cfg(test)]

use approx::assert_relative_eq;
use integration_test_shader::buffer_barriers::{CopyParam, COMPUTE_COPY_WG};
use rust_gpu_bindless::descriptor::boxed::BoxMutBufferExt;
use rust_gpu_bindless::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, DescriptorCounts,
	MutDescBufferExt,
};
use rust_gpu_bindless::pipeline::access_type::{Read, ReadWrite};
use rust_gpu_bindless::pipeline::compute_pipeline::BindlessComputePipeline;
use rust_gpu_bindless::platform::ash::{
	ash_init_single_graphics_queue, Ash, AshSingleGraphicsQueueCreateInfo, Debuggers,
};
use rust_gpu_bindless::platform::{BindlessPipelinePlatform, ExecutingContext, RecordingContext};
use std::sync::Arc;

#[test]
fn test_buffer_barrier_ash() -> anyhow::Result<()> {
	unsafe {
		let bindless = Bindless::<Ash>::new(
			ash_init_single_graphics_queue(AshSingleGraphicsQueueCreateInfo {
				debug: Debuggers::None,
				..AshSingleGraphicsQueueCreateInfo::default()
			})?,
			DescriptorCounts::REASONABLE_DEFAULTS,
		);
		test_buffer_barrier(&bindless)
	}
}

fn test_buffer_barrier<P: BindlessPipelinePlatform>(bindless: &Arc<Bindless<P>>) -> anyhow::Result<()> {
	let value = (0..1024).map(|i| i as f32).collect::<Vec<_>>();
	let len = value.len();

	let buffer_ci = |name: &'static str| BindlessBufferCreateInfo {
		name,
		usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::MAP_READ | BindlessBufferUsage::STORAGE_BUFFER,
		allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
	};
	let first = bindless
		.buffer()
		.alloc_from_iter(&buffer_ci("first"), value.iter().copied())?;
	let second = bindless.buffer().alloc_slice(&buffer_ci("second"), len)?;
	let third = bindless.buffer().alloc_slice(&buffer_ci("third"), len)?;

	let compute = BindlessComputePipeline::new(bindless, crate::shader::buffer_barriers::compute_copy::new())?;
	let third = bindless
		.execute(|cmd| unsafe {
			let first = first.access_dont_care_unchecked(cmd).transition::<Read>();
			let second = second.access_dont_care_unchecked(cmd).transition::<ReadWrite>();

			let wgs = (len as u32 + COMPUTE_COPY_WG - 1) / COMPUTE_COPY_WG;
			cmd.dispatch(
				&compute,
				[wgs, 1, 1],
				CopyParam {
					input: first.to_transient(),
					output: second.to_mut_transient(),
					len: len as u32,
				},
			)?;

			let second = second.transition::<Read>();
			let third = third.access_dont_care_unchecked(cmd).transition::<ReadWrite>();

			cmd.dispatch(
				&compute,
				[wgs, 1, 1],
				CopyParam {
					input: second.to_transient(),
					output: third.to_mut_transient(),
					len: len as u32,
				},
			)?;

			Ok(third.into_inner())
		})?
		.block_on();

	let result = unsafe { third.mapped_unchecked()?.read_iter().collect::<Vec<_>>() };
	assert_relative_eq!(&*result, &*value, epsilon = 0.01);
	Ok(())
}
