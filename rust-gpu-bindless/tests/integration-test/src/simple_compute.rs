#![cfg(test)]

use integration_test_shader::simple_compute::{add_calculation, Param};
use rust_gpu_bindless::descriptor::boxed::{BoxDescExt, MutBoxDescExt};
use rust_gpu_bindless::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, DescriptorCounts,
	MutDescBufferExt, RCDescExt,
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
		test_add_single(&bindless)
	}
}

fn test_add_single<P: BindlessPipelinePlatform>(bindless: &Arc<Bindless<P>>) -> anyhow::Result<()> {
	let a = 42;
	let b = 69;
	let c = 123;

	// Pipelines can be created from the shaders and carry the `T` generic which is the param struct of the shader.
	let pipeline = BindlessComputePipeline::new(&bindless, crate::shader::simple_compute::new())?;

	// create buffers with initial content
	let buffer_ci = |name| BindlessBufferCreateInfo {
		name,
		usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::STORAGE_BUFFER,
		allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
	};
	let buffer_a = bindless.buffer().alloc_from_data(&buffer_ci("a"), a)?.into_shared();
	let buffer_b = bindless.buffer().alloc_from_data(&buffer_ci("b"), b)?.into_shared();

	let execution_context = bindless.execute(|recording_context| {
		// a and b are read-only accessors to their respective buffers that only live for as long as
		// (the lifetime 'a on) recording_context does. By passing in recording_context by reference, it is ensured
		// you can't leak the accessors outside this block (apart from reentrant recording)
		let a = buffer_a.to_transient(recording_context);
		let b = buffer_b.to_transient(recording_context);

		// Mutable resources can only be used unsafely, the user has to ensure they aren't used by multiple
		// recordings simultaneously. By creating the output buffer here and returning it, it can only be accessed
		// once the execution has completed.
		// While I'd love to design a safe api around this, I simply don't have the time for it right now.
		let buffer_out = bindless
			.buffer()
			.alloc_from_data(
				&BindlessBufferCreateInfo {
					name: "c readback",
					usage: BindlessBufferUsage::MAP_READ
						| BindlessBufferUsage::MAP_WRITE
						| BindlessBufferUsage::STORAGE_BUFFER,
					allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
				},
				0,
			)
			.unwrap();
		let out = unsafe { buffer_out.to_transient_unchecked(recording_context) };

		// Enqueueing some dispatch takes in a user-supplied param struct that may contain any number of buffer
		// accesses. This method will internally "remove" the lifetime of the param struct, as the lifetime of the
		// buffers will be ensured by this execution not having finished yet.
		// Note how `c` is passed in directly, without an indirection like the other buffers.
		recording_context.dispatch(&pipeline, [1, 1, 1], Param { a, b, c, out })?;

		// you can return arbitrary data here, that can only be accessed once the execution has finished
		Ok(buffer_out)

		// returning makes us loose the reference on recording_context, so no accessors can leak beyond here
	})?;

	// wait for result and get back the data returned from the closure
	let mut out = execution_context.block_on();

	// unsafely map the buffer to read data from it. Just like the executing, the end user has to ensure no concurrent
	// access by some other execution or map operation.
	let result = unsafe { out.mapped()?.read_data() };
	println!("result: {:?}", result);
	assert_eq!(result, add_calculation(a, b, c));
	Ok(())
}
