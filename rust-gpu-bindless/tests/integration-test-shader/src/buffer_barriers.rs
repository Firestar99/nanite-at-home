use glam::UVec3;
use rust_gpu_bindless_macros::{BufferStruct, bindless};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Descriptors, MutBuffer, TransientDesc};
use static_assertions::const_assert_eq;

#[derive(Copy, Clone, BufferStruct)]
pub struct CopyParam<'a> {
	pub input: TransientDesc<'a, Buffer<[f32]>>,
	pub output: TransientDesc<'a, MutBuffer<[f32]>>,
	pub len: u32,
}

pub const COMPUTE_COPY_WG: u32 = 64;

const_assert_eq!(COMPUTE_COPY_WG, 64);
#[bindless(compute(threads(64)))]
pub fn compute_copy(
	#[bindless(descriptors)] mut descriptors: Descriptors<'_>,
	#[bindless(param)] param: &CopyParam<'static>,
	#[spirv(workgroup_id)] wg_id: UVec3,
	#[spirv(local_invocation_id)] inv_id: UVec3,
) {
	unsafe {
		let index = wg_id.x * COMPUTE_COPY_WG + inv_id.x;
		if index < param.len {
			let t = param.input.access(&descriptors).load(index as usize);
			param.output.access(&mut descriptors).store(index as usize, t);
		}
	}
}
