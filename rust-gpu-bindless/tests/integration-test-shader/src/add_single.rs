use rust_gpu_bindless_macros::{bindless, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Descriptors, MutBuffer, TransientDesc};

#[derive(Copy, Clone, BufferStruct)]
pub struct Param<'a> {
	pub a: TransientDesc<'a, Buffer<u32>>,
	pub b: TransientDesc<'a, Buffer<u32>>,
	pub c: TransientDesc<'a, MutBuffer<u32>>,
}

// wg of 1 is silly slow but doesn't matter
#[bindless(compute(threads(1)))]
pub fn add_compute(
	#[bindless(descriptors)] mut descriptors: Descriptors<'_>,
	#[bindless(param)] param: &Param<'static>,
) {
	let a = param.a.access(&descriptors).load();
	let b = param.b.access(&descriptors).load();
	let c = add_calculation(a, b);
	unsafe {
		param.c.access(&mut descriptors).store(c);
	}
}

pub fn add_calculation(a: u32, b: u32) -> u32 {
	a + b
}
