use glam::UVec3;
use rust_gpu_bindless_macros::{BufferStruct, bindless};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Descriptors, MutBuffer, StrongDesc, TransientDesc};

#[derive(Copy, Clone, BufferStruct)]
pub struct Indirection {
	pub c: StrongDesc<Buffer<f32>>,
}

#[derive(Copy, Clone, BufferStruct)]
pub struct Param<'a> {
	pub a: f32,
	pub b: TransientDesc<'a, Buffer<[f32]>>,
	pub indirection: TransientDesc<'a, Buffer<Indirection>>,
	pub out: TransientDesc<'a, MutBuffer<[f32]>>,
}

// wg of 1 is silly slow but doesn't matter
#[bindless(compute(threads(1)))]
pub fn simple_compute(
	#[bindless(descriptors)] mut descriptors: Descriptors<'_>,
	#[bindless(param)] param: &Param<'static>,
	#[spirv(workgroup_id)] wg_id: UVec3,
) {
	unsafe {
		let a = param.a;

		let index = wg_id.x as usize;
		let b = param.b.access(&descriptors).load(index);

		let indirection = param.indirection.access(&descriptors).load();
		let c = indirection.c.access(&descriptors).load();

		let result = add_calculation(a, b, c);
		param.out.access(&mut descriptors).store(index, result);
	}
}

pub fn add_calculation(a: f32, b: f32, c: f32) -> f32 {
	a * b + c
}
