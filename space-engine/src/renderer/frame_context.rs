use rust_gpu_bindless::descriptor::{
	BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, Buffer, RCDescExt, TransientDesc,
};
use rust_gpu_bindless::pipeline::{HasResourceContext, Recording};
use space_engine_shader::renderer::frame_data::FrameData;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// A `FrameContext` is created once per Frame rendered, containing frame-specific information and access to resources.
pub struct FrameContext<'a> {
	pub frame_data: FrameData,
	pub frame_data_desc: TransientDesc<'a, Buffer<FrameData>>,
	_private: PhantomData<()>,
}

impl<'a> FrameContext<'a> {
	pub fn new(cmd: &Recording<'a>, frame_data: FrameData) -> anyhow::Result<FrameContext<'a>> {
		let bindless = cmd.bindless();
		Ok(Self {
			frame_data,
			frame_data_desc: bindless
				.buffer()
				.alloc_shared_from_data(
					&BindlessBufferCreateInfo {
						usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::STORAGE_BUFFER,
						name: "FrameData",
						allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
					},
					frame_data,
				)?
				.to_transient(cmd),
			_private: PhantomData,
		})
	}
}

impl<'a> Deref for FrameContext<'a> {
	type Target = FrameData;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.frame_data
	}
}

impl<'a> DerefMut for FrameContext<'a> {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.frame_data
	}
}
