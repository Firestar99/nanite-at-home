#![cfg(test)]

use integration_test_shader::add_single::Param;
use rust_gpu_bindless::descriptor::mutable::MutDescExt;
use rust_gpu_bindless::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, DescriptorCounts, RCDescExt,
};
use rust_gpu_bindless::pipeline::compute_pipeline::BindlessComputePipeline;
use rust_gpu_bindless::platform::ash::{ash_init_single_graphics_queue, Ash, AshSingleGraphicsQueueCreateInfo};
use rust_gpu_bindless::platform::{BindlessPipelinePlatform, ExecutingCommandBuffer, RecordingCommandBuffer};
use std::sync::Arc;

#[test]
fn test_add_single_ash() -> anyhow::Result<()> {
	unsafe {
		let bindless = Bindless::<Ash>::new(
			ash_init_single_graphics_queue(AshSingleGraphicsQueueCreateInfo {
				..AshSingleGraphicsQueueCreateInfo::default()
			})?,
			DescriptorCounts::REASONABLE_DEFAULTS,
		);
		test_add_single(&bindless);
		Ok(())
	}
}

fn test_add_single<P: BindlessPipelinePlatform>(bindless: &Arc<Bindless<P>>) {
	let pipeline = BindlessComputePipeline::new(&bindless, crate::shader::add_single::new()).unwrap();

	let buffer_ci = BindlessBufferCreateInfo {
		name: "",
		usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::STORAGE_BUFFER,
		allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
	};
	let buffer_a = bindless.buffer().alloc_from_data(&buffer_ci, 42).unwrap().into_shared();
	let buffer_b = bindless.buffer().alloc_from_data(&buffer_ci, 69).unwrap().into_shared();
	let execution_context = bindless
		.execute(|recording_context| {
			// buffer_* are read-only accesses to their respective buffers that only live for
			// as long as recording_context does. By creating them, it is ensured that the buffer
			// stays alive for at least as long as recording_context does.
			let a = buffer_a.to_transient(recording_context);
			let b = buffer_b.to_transient(recording_context);

			// Mutable resources can only be used unsafely, the user has to ensure they aren't used by multiple recordings
			// simultaneously. By creating the output buffer here and returning it, it can only be accessed once the
			// execution has completed.
			// let buffer_c = bindless.buffer().alloc_from_data(&BindlessBufferCreateInfo {
			// 	name: "c readback",
			// 	usage: BindlessBufferUsage::MAP_READ | BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::STORAGE_BUFFER,
			// 	allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
			// }, 0).unwrap();
			// let c = unsafe { buffer_c.to_mut_transient_unchecked(&recording_context) };

			// Enqueueing some dispatch takes in a user-supplied param struct that may contain
			// any number of buffer accesses. This method will internally "remove" the lifetime
			// of the param struct, the lifetime of the buffers is now ensured by this execution
			// not having finished yet.
			recording_context.dispatch(
				&pipeline,
				[1, 1, 1],
				Param {
					a,
					b,
					// c,
				},
			)?;
			// Submit consumes self, so buffer_a and buffer_b cannot be used beyond here. Will
			// return some kind of object to track when the execution finished, but could also
			// be fire and forget.
			// buffer_c
			Ok(())
		})
		.unwrap();
	execution_context.block_on();
}
