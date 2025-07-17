use crate::descriptor::{
	AddressMode, BindlessAllocationScheme, BindlessBufferUsage, BindlessImageUsage, BorderColor, Extent, Filter,
	SampleCount,
};
use crate::pipeline::{ClearValue, ImageAccessType, IndexType, LoadOp, RenderingAttachment, StoreOp};
use crate::platform::ash::Ash;
use ash::vk::{
	AttachmentLoadOp, AttachmentStoreOp, Extent2D, ImageLayout, ImageType as VkImageType, RenderingAttachmentInfo,
	ShaderStageFlags,
};
use ash::vk::{BufferUsageFlags, Extent3D, ImageUsageFlags, ImageViewType, SampleCountFlags};
use gpu_allocator::MemoryLocation;
use gpu_allocator::vulkan::AllocationScheme;
use rust_gpu_bindless_shaders::descriptor::ImageType;
use rust_gpu_bindless_shaders::shader_type::Shader;
use spirv_std::image::{Arrayed, Dimensionality};

impl BindlessAllocationScheme {
	pub fn to_gpu_allocator_buffer(&self, buffer: ash::vk::Buffer) -> AllocationScheme {
		match self {
			BindlessAllocationScheme::Dedicated => AllocationScheme::DedicatedBuffer(buffer),
			BindlessAllocationScheme::AllocatorManaged => AllocationScheme::GpuAllocatorManaged,
		}
	}

	pub fn to_gpu_allocator_image(&self, image: ash::vk::Image) -> AllocationScheme {
		match self {
			BindlessAllocationScheme::Dedicated => AllocationScheme::DedicatedImage(image),
			BindlessAllocationScheme::AllocatorManaged => AllocationScheme::GpuAllocatorManaged,
		}
	}
}

impl BindlessBufferUsage {
	pub fn to_ash_buffer_usage_flags(&self) -> BufferUsageFlags {
		let mut out = BufferUsageFlags::empty();
		if self.contains(BindlessBufferUsage::TRANSFER_SRC) {
			out |= BufferUsageFlags::TRANSFER_SRC;
		}
		if self.contains(BindlessBufferUsage::TRANSFER_DST) {
			out |= BufferUsageFlags::TRANSFER_DST;
		}
		if self.contains(BindlessBufferUsage::UNIFORM_BUFFER) {
			out |= BufferUsageFlags::UNIFORM_BUFFER;
		}
		if self.contains(BindlessBufferUsage::STORAGE_BUFFER) {
			out |= BufferUsageFlags::STORAGE_BUFFER;
		}
		if self.contains(BindlessBufferUsage::INDEX_BUFFER) {
			out |= BufferUsageFlags::INDEX_BUFFER;
		}
		if self.contains(BindlessBufferUsage::VERTEX_BUFFER) {
			out |= BufferUsageFlags::VERTEX_BUFFER;
		}
		if self.contains(BindlessBufferUsage::INDIRECT_BUFFER) {
			out |= BufferUsageFlags::INDIRECT_BUFFER;
		}
		// empty flags are invalid in vulkan, this is reachable via a buffer that is only host mappable
		assert!(!self.is_empty());
		if out.is_empty() {
			BufferUsageFlags::TRANSFER_SRC
		} else {
			out
		}
	}

	/// prioritizes MAP_WRITE over MAP_READ
	pub fn to_gpu_allocator_memory_location(&self) -> MemoryLocation {
		if self.contains(BindlessBufferUsage::MAP_WRITE) {
			MemoryLocation::CpuToGpu
		} else if self.contains(BindlessBufferUsage::MAP_READ) {
			MemoryLocation::GpuToCpu
		} else {
			MemoryLocation::GpuOnly
		}
	}
}

pub fn bindless_image_type_to_vk_image_type<T: ImageType>() -> Option<VkImageType> {
	match T::dimensionality() {
		Dimensionality::OneD => Some(VkImageType::TYPE_1D),
		Dimensionality::TwoD => Some(VkImageType::TYPE_2D),
		Dimensionality::ThreeD => Some(VkImageType::TYPE_3D),
		Dimensionality::Cube => Some(VkImageType::TYPE_2D),
		Dimensionality::Rect => Some(VkImageType::TYPE_2D),
		Dimensionality::Buffer => None,
		Dimensionality::SubpassData => None,
	}
}

pub fn bindless_image_type_to_vk_image_view_type<T: ImageType>() -> Option<ImageViewType> {
	match (T::dimensionality(), T::arrayed()) {
		(Dimensionality::OneD, Arrayed::False) => Some(ImageViewType::TYPE_1D),
		(Dimensionality::OneD, Arrayed::True) => Some(ImageViewType::TYPE_1D_ARRAY),
		(Dimensionality::TwoD, Arrayed::False) => Some(ImageViewType::TYPE_2D),
		(Dimensionality::TwoD, Arrayed::True) => Some(ImageViewType::TYPE_2D_ARRAY),
		(Dimensionality::ThreeD, Arrayed::False) => Some(ImageViewType::TYPE_3D),
		(Dimensionality::ThreeD, Arrayed::True) => None,
		(Dimensionality::Cube, Arrayed::False) => Some(ImageViewType::CUBE),
		(Dimensionality::Cube, Arrayed::True) => Some(ImageViewType::CUBE_ARRAY),
		(Dimensionality::Rect, Arrayed::False) => Some(ImageViewType::TYPE_2D),
		(Dimensionality::Rect, Arrayed::True) => Some(ImageViewType::TYPE_2D_ARRAY),
		(Dimensionality::Buffer, _) => None,
		(Dimensionality::SubpassData, _) => None,
	}
}

impl SampleCount {
	pub fn to_ash_sample_count_flags(&self) -> SampleCountFlags {
		match self {
			SampleCount::Sample1 => SampleCountFlags::TYPE_1,
			SampleCount::Sample2 => SampleCountFlags::TYPE_2,
			SampleCount::Sample4 => SampleCountFlags::TYPE_4,
			SampleCount::Sample8 => SampleCountFlags::TYPE_8,
			SampleCount::Sample16 => SampleCountFlags::TYPE_16,
			SampleCount::Sample32 => SampleCountFlags::TYPE_32,
			SampleCount::Sample64 => SampleCountFlags::TYPE_64,
		}
	}
}

impl From<Extent> for Extent3D {
	fn from(value: Extent) -> Self {
		Extent3D {
			width: value.width,
			height: value.height,
			depth: value.depth,
		}
	}
}

impl From<Extent> for Extent2D {
	fn from(value: Extent) -> Self {
		Extent2D {
			width: value.width,
			height: value.height,
		}
	}
}

impl BindlessImageUsage {
	pub fn to_ash_image_usage_flags(&self) -> ImageUsageFlags {
		let mut out = ImageUsageFlags::empty();
		if self.contains(BindlessImageUsage::TRANSFER_SRC) {
			out |= ImageUsageFlags::TRANSFER_SRC;
		}
		if self.contains(BindlessImageUsage::TRANSFER_DST) {
			out |= ImageUsageFlags::TRANSFER_DST;
		}
		if self.contains(BindlessImageUsage::SAMPLED) {
			out |= ImageUsageFlags::SAMPLED;
		}
		if self.contains(BindlessImageUsage::STORAGE) {
			out |= ImageUsageFlags::STORAGE;
		}
		if self.contains(BindlessImageUsage::COLOR_ATTACHMENT) {
			out |= ImageUsageFlags::COLOR_ATTACHMENT;
		}
		if self.contains(BindlessImageUsage::DEPTH_STENCIL_ATTACHMENT) {
			out |= ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
		}
		// empty flags are invalid in vulkan, but unlike buffer this is unreachable
		assert!(!out.is_empty());
		out
	}

	pub fn to_gpu_allocator_memory_location(&self) -> MemoryLocation {
		MemoryLocation::GpuOnly
	}

	pub fn has_image_view(&self) -> bool {
		self.intersects(
			BindlessImageUsage::SAMPLED
				| BindlessImageUsage::STORAGE
				| BindlessImageUsage::COLOR_ATTACHMENT
				| BindlessImageUsage::DEPTH_STENCIL_ATTACHMENT,
		)
	}
}

pub trait ShaderAshExt {
	fn to_ash_shader_stage(&self) -> ShaderStageFlags;
}

impl ShaderAshExt for Shader {
	fn to_ash_shader_stage(&self) -> ShaderStageFlags {
		match self {
			Shader::VertexShader => ShaderStageFlags::VERTEX,
			Shader::TesselationControlShader => ShaderStageFlags::TESSELLATION_CONTROL,
			Shader::TesselationEvaluationShader => ShaderStageFlags::TESSELLATION_EVALUATION,
			Shader::GeometryShader => ShaderStageFlags::GEOMETRY,
			Shader::FragmentShader => ShaderStageFlags::FRAGMENT,
			Shader::ComputeShader => ShaderStageFlags::COMPUTE,
			Shader::TaskShader => ShaderStageFlags::TASK_EXT,
			Shader::MeshShader => ShaderStageFlags::MESH_EXT,
		}
	}
}

impl LoadOp {
	pub fn to_ash(&self) -> AttachmentLoadOp {
		match self {
			LoadOp::Load => AttachmentLoadOp::LOAD,
			LoadOp::Clear(_) => AttachmentLoadOp::CLEAR,
			LoadOp::DontCare => AttachmentLoadOp::DONT_CARE,
		}
	}

	pub fn to_ash_clear_color(&self) -> ash::vk::ClearValue {
		match self {
			LoadOp::Clear(c) => c.to_ash(),
			LoadOp::Load | LoadOp::DontCare => ash::vk::ClearValue::default(),
		}
	}
}

impl StoreOp {
	pub fn to_ash(&self) -> AttachmentStoreOp {
		match self {
			StoreOp::Store => AttachmentStoreOp::STORE,
			StoreOp::DontCare => AttachmentStoreOp::DONT_CARE,
		}
	}
}

impl ClearValue {
	pub fn to_ash(&self) -> ash::vk::ClearValue {
		let mut ret = ash::vk::ClearValue::default();
		match *self {
			ClearValue::ColorF(a) => ret.color.float32 = a,
			ClearValue::ColorU(a) => ret.color.uint32 = a,
			ClearValue::ColorI(a) => ret.color.int32 = a,
			ClearValue::DepthStencil { depth, stencil } => {
				ret.depth_stencil.depth = depth;
				ret.depth_stencil.stencil = stencil;
			}
		}
		ret
	}
}

impl<A: ImageAccessType> RenderingAttachment<'_, '_, Ash, A> {
	pub unsafe fn to_ash(&self, layout: ImageLayout) -> RenderingAttachmentInfo<'_> {
		unsafe {
			RenderingAttachmentInfo::default()
				.image_view(self.image.inner_slot().image_view.unwrap())
				.image_layout(layout)
				.load_op(self.load_op.to_ash())
				.store_op(self.store_op.to_ash())
				.clear_value(self.load_op.to_ash_clear_color())
		}
	}
}

impl IndexType {
	pub fn to_ash_index_type(&self) -> ash::vk::IndexType {
		match self {
			IndexType::U32 => ash::vk::IndexType::UINT32,
			IndexType::U16 => ash::vk::IndexType::UINT16,
		}
	}
}

impl Filter {
	pub fn to_ash_filter(&self) -> ash::vk::Filter {
		match self {
			Filter::Nearest => ash::vk::Filter::NEAREST,
			Filter::Linear => ash::vk::Filter::LINEAR,
		}
	}

	pub fn to_ash_mipmap_mode(&self) -> ash::vk::SamplerMipmapMode {
		match self {
			Filter::Nearest => ash::vk::SamplerMipmapMode::NEAREST,
			Filter::Linear => ash::vk::SamplerMipmapMode::LINEAR,
		}
	}
}

impl AddressMode {
	pub fn to_ash_address_mode(&self) -> ash::vk::SamplerAddressMode {
		match self {
			AddressMode::ClampToEdge => ash::vk::SamplerAddressMode::CLAMP_TO_EDGE,
			AddressMode::Repeat => ash::vk::SamplerAddressMode::REPEAT,
			AddressMode::MirrorRepeat => ash::vk::SamplerAddressMode::MIRRORED_REPEAT,
			AddressMode::ClampToBorder => ash::vk::SamplerAddressMode::CLAMP_TO_BORDER,
		}
	}
}

impl BorderColor {
	pub fn to_ash_border_color(&self, is_integer: bool) -> ash::vk::BorderColor {
		match (self, is_integer) {
			(BorderColor::TransparentBlack, false) => ash::vk::BorderColor::FLOAT_TRANSPARENT_BLACK,
			(BorderColor::TransparentBlack, true) => ash::vk::BorderColor::INT_TRANSPARENT_BLACK,
			(BorderColor::OpaqueBlack, false) => ash::vk::BorderColor::FLOAT_OPAQUE_BLACK,
			(BorderColor::OpaqueBlack, true) => ash::vk::BorderColor::INT_OPAQUE_BLACK,
			(BorderColor::OpaqueWhite, false) => ash::vk::BorderColor::FLOAT_OPAQUE_WHITE,
			(BorderColor::OpaqueWhite, true) => ash::vk::BorderColor::INT_OPAQUE_WHITE,
		}
	}
}
