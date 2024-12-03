use rust_gpu_bindless_macros::{bindless, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Descriptors, TransientDesc};

#[derive(Copy, Clone, BufferStruct)]
pub struct Param<'a> {
	pub a: TransientDesc<'a, Buffer<u32>>,
	pub b: TransientDesc<'a, Buffer<u32>>,
}

// wg of 1 is silly slow but doesn't matter
#[bindless(compute(threads(1)))]
pub fn add(#[bindless(descriptors)] descriptors: &Descriptors, #[bindless(param)] param: &Param<'static>) {
	let a = param.a.access(descriptors).load();
	let b = param.b.access(descriptors).load();
	let c = a + b;
	panic!("{} + {} = {}", a, b, c);
}
