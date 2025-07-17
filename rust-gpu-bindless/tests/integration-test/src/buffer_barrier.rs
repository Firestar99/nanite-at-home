#![cfg(test)]

use crate::debugger;
use approx::assert_relative_eq;
use integration_test_shader::buffer_barriers::{COMPUTE_COPY_WG, CopyParam};
use pollster::block_on;
use rust_gpu_bindless_core::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessInstance,
	DescriptorCounts, MutDescBufferExt,
};
use rust_gpu_bindless_core::pipeline::{HostAccess, MutBufferAccessExt, ShaderRead, ShaderReadWrite};
use rust_gpu_bindless_core::platform::BindlessPipelinePlatform;
use rust_gpu_bindless_core::platform::ash::{Ash, AshSingleGraphicsQueueCreateInfo, ash_init_single_graphics_queue};

#[test]
fn test_buffer_barrier_ash() -> anyhow::Result<()> {
	unsafe {
		let bindless = BindlessInstance::<Ash>::new(
			ash_init_single_graphics_queue(AshSingleGraphicsQueueCreateInfo {
				debug: debugger(),
				..AshSingleGraphicsQueueCreateInfo::default()
			})?,
			DescriptorCounts::REASONABLE_DEFAULTS,
		);
		block_on(test_buffer_barrier(&bindless))?;
		Ok(())
	}
}

async fn test_buffer_barrier<P: BindlessPipelinePlatform>(bindless: &Bindless<P>) -> anyhow::Result<()> {
	let value = (0..1024).map(|i| i as f32).collect::<Vec<_>>();
	let len = value.len();

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
	let third = bindless.execute(|cmd| unsafe {
		let first = first.access::<ShaderRead>(cmd)?;
		let second = second.access_as_undefined::<ShaderReadWrite>(cmd)?;

		// 2. does a dispatch to copy from `first` to `second`
		let wgs = (len as u32).div_ceil(COMPUTE_COPY_WG);
		cmd.dispatch(
			&compute,
			[wgs, 1, 1],
			CopyParam {
				input: first.to_transient()?,
				output: second.to_mut_transient()?,
				len: len as u32,
			},
		)?;

		// 3. adds some barriers to ensure the data just written in `second` is visible in the next operation
		let second = second.transition::<ShaderRead>()?;
		let third = third.access_as_undefined::<ShaderReadWrite>(cmd)?;

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
	})?;

	// 5. downloads the data from `third` and verifies that it hasn't corrupted
	let result = third.mapped().await?.read_iter().collect::<Vec<_>>();
	assert_relative_eq!(&*result, &*value, epsilon = 0.01);
	Ok(())
}
