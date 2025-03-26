/// The primary platform is Ash
#[cfg(feature = "ash")]
pub type P = rust_gpu_bindless_core::platform::ash::Ash;
#[cfg(not(any(feature = "ash")))]
compile_error!("Must select a primary platform by enabling a feature like \"ash\"");

pub mod backing {
	pub use rust_gpu_bindless_core::backing::*;
}

pub mod descriptor {
	pub type Bindless = rust_gpu_bindless_core::descriptor::Bindless<crate::P>;
	pub type BindlessInstance = rust_gpu_bindless_core::descriptor::BindlessInstance<crate::P>;
	pub type BindlessFrame = rust_gpu_bindless_core::descriptor::BindlessFrame<crate::P>;
	pub type StrongMetadataCpu<'a> = rust_gpu_bindless_core::descriptor::StrongMetadataCpu<'a, crate::P>;
	pub type BackingRefsError = rust_gpu_bindless_core::descriptor::BackingRefsError;
	pub type BufferTable = rust_gpu_bindless_core::descriptor::BufferTable<crate::P>;
	pub type BufferSlot = rust_gpu_bindless_core::descriptor::BufferSlot<crate::P>;
	pub type BufferTableAccess<'a> = rust_gpu_bindless_core::descriptor::BufferTableAccess<'a, crate::P>;
	pub type BufferAllocationError = rust_gpu_bindless_core::descriptor::BufferAllocationError<crate::P>;
	pub type BufferInterface = rust_gpu_bindless_core::descriptor::BufferInterface<crate::P>;
	pub type StrongBackingRefs = rust_gpu_bindless_core::descriptor::StrongBackingRefs<crate::P>;
	pub type ImageTable = rust_gpu_bindless_core::descriptor::ImageTable<crate::P>;
	pub type ImageSlot = rust_gpu_bindless_core::descriptor::ImageSlot<crate::P>;
	pub type ImageTableAccess<'a> = rust_gpu_bindless_core::descriptor::ImageTableAccess<'a, crate::P>;
	pub type ImageAllocationError = rust_gpu_bindless_core::descriptor::ImageAllocationError<crate::P>;
	pub type ImageInterface = rust_gpu_bindless_core::descriptor::ImageInterface<crate::P>;
	pub type Mut = rust_gpu_bindless_core::descriptor::Mut<crate::P>;
	pub type MutDesc<C> = rust_gpu_bindless_core::descriptor::MutDesc<crate::P, C>;
	pub type RC = rust_gpu_bindless_core::descriptor::RC<crate::P>;
	pub type RCDesc<C> = rust_gpu_bindless_core::descriptor::RCDesc<crate::P, C>;
	pub type AnyRCDesc = rust_gpu_bindless_core::descriptor::AnyRCDesc<crate::P>;
	pub type SamplerTable = rust_gpu_bindless_core::descriptor::SamplerTable<crate::P>;
	pub type SamplerTableAccess<'a> = rust_gpu_bindless_core::descriptor::SamplerTableAccess<'a, crate::P>;
	pub type SamplerInterface = rust_gpu_bindless_core::descriptor::SamplerInterface<crate::P>;

	pub use rust_gpu_bindless_core::descriptor::*;
}

pub mod pipeline {
	pub type MutBufferAccess<'a, T, A> = rust_gpu_bindless_core::pipeline::MutBufferAccess<'a, crate::P, T, A>;
	pub type MutImageAccess<'a, T, A> = rust_gpu_bindless_core::pipeline::MutImageAccess<'a, crate::P, T, A>;
	pub type BindlessComputePipeline<T> = rust_gpu_bindless_core::pipeline::BindlessComputePipeline<crate::P, T>;
	pub type BindlessGraphicsPipeline<T> = rust_gpu_bindless_core::pipeline::BindlessGraphicsPipeline<crate::P, T>;
	pub type BindlessMeshGraphicsPipeline<T> =
		rust_gpu_bindless_core::pipeline::BindlessMeshGraphicsPipeline<crate::P, T>;
	pub type RecordingError = rust_gpu_bindless_core::pipeline::RecordingError<crate::P>;
	pub type Recording<'a> = rust_gpu_bindless_core::pipeline::Recording<'a, crate::P>;
	pub type Rendering<'a, 'b> = rust_gpu_bindless_core::pipeline::Rendering<'a, 'b, crate::P>;
	pub type RenderingAttachment<'a, 'b, A> =
		rust_gpu_bindless_core::pipeline::RenderingAttachment<'a, 'b, crate::P, A>;

	pub use rust_gpu_bindless_core::pipeline::*;
}

pub mod platform {
	pub use rust_gpu_bindless_core::platform::*;
}

pub mod __private {
	pub use rust_gpu_bindless_core::__private::*;
}
