#![cfg(test)]

use approx::assert_relative_eq;
use integration_test_shader::buffer_barriers::{CopyParam, COMPUTE_COPY_WG};
use rust_gpu_bindless::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, DescriptorCounts,
	MutDescBufferExt,
};
use rust_gpu_bindless::pipeline::access_buffer::MutBufferAccessExt;
use rust_gpu_bindless::pipeline::access_type::{HostAccess, ShaderRead, ShaderReadWrite};
use rust_gpu_bindless::platform::ash::{
	ash_init_single_graphics_queue, Ash, AshSingleGraphicsQueueCreateInfo, Debuggers,
};
use rust_gpu_bindless::platform::{BindlessPipelinePlatform, ExecutingContext};
use std::sync::Arc;

#[test]
fn test_semaphore_ash() -> anyhow::Result<()> {
	unsafe {
		let bindless = Bindless::<Ash>::new(
			ash_init_single_graphics_queue(AshSingleGraphicsQueueCreateInfo {
				debug: Debuggers::Validation,
				..AshSingleGraphicsQueueCreateInfo::default()
			})?,
			DescriptorCounts::REASONABLE_DEFAULTS,
		);
		test_semaphore(&bindless)
	}
}

fn test_semaphore<P: BindlessPipelinePlatform>(bindless: &Arc<Bindless<P>>) -> anyhow::Result<()> {
	let value = (0..1024).map(|i| i as f32).collect::<Vec<_>>();
	let len = value.len();
	let wgs = (len as u32 + COMPUTE_COPY_WG - 1) / COMPUTE_COPY_WG;

	// The shader isn't very interesting, it just copies data from `input` to `output`.
	// Rather have a look at this CPU code, which...
	let buffer_ci = |name: &'static str| BindlessBufferCreateInfo {
		name,
		usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::MAP_READ | BindlessBufferUsage::STORAGE_BUFFER,
		allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
	};
	// 1. uploads some data into `first`
	let first = bindless
		.buffer()
		.alloc_from_iter(&buffer_ci("first"), value.iter().copied())?;
	let second = bindless.buffer().alloc_slice(&buffer_ci("second"), len)?;
	let third = bindless.buffer().alloc_slice(&buffer_ci("third"), len)?;

	let compute = bindless.create_compute_pipeline(crate::shader::buffer_barriers::compute_copy::new())?;
	let second = bindless
		.execute(|cmd| unsafe {
			let first = first.access::<ShaderRead>(cmd)?;
			let second = second.access_undefined_contents::<ShaderReadWrite>(cmd)?;

			// 2. does a dispatch to copy from `first` to `second`
			cmd.dispatch(
				&compute,
				[wgs, 1, 1],
				CopyParam {
					input: first.to_transient()?,
					output: second.to_mut_transient()?,
					len: len as u32,
				},
			)?;

			Ok(second.into_desc())
		})?
		.block_on();

	let third = bindless
		.execute(|cmd| unsafe {
			// 3. adds some barriers to ensure the data just written in `second` is visible in the next operation
			let second = second.access::<ShaderRead>(cmd)?;
			let third = third.access_undefined_contents::<ShaderReadWrite>(cmd)?;

			// 4. another dispatch to copy from `second` to `third`
			cmd.dispatch(
				&compute,
				[wgs, 1, 1],
				CopyParam {
					input: second.to_transient()?,
					output: third.to_mut_transient()?,
					len: len as u32,
				},
			)?;

			Ok(third.transition::<HostAccess>()?.into_desc())
		})?
		.block_on();

	// 5. downloads the data from `third` and verifies that it hasn't corrupted
	let result = third.mapped()?.read_iter().collect::<Vec<_>>();
	assert_relative_eq!(&*result, &*value, epsilon = 0.01);
	Ok(())
}
