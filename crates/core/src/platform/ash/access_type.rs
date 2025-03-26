use crate::pipeline::{BufferAccess, ImageAccess};
use ash::vk::{AccessFlags2, ImageLayout, PipelineStageFlags2};

pub struct AshBufferAccess {
	pub stage_mask: PipelineStageFlags2,
	pub access_mask: AccessFlags2,
}

impl AshBufferAccess {
	pub const fn new(stage_mask: PipelineStageFlags2, access_mask: AccessFlags2) -> Self {
		Self {
			stage_mask,
			access_mask,
		}
	}
}

impl BufferAccess {
	pub fn to_ash_buffer_access(&self) -> AshBufferAccess {
		match self {
			BufferAccess::Undefined => AshBufferAccess::new(PipelineStageFlags2::ALL_COMMANDS, AccessFlags2::NONE),
			BufferAccess::General => AshBufferAccess::new(
				PipelineStageFlags2::ALL_COMMANDS,
				AccessFlags2::MEMORY_READ | AccessFlags2::MEMORY_WRITE,
			),
			BufferAccess::GeneralRead => {
				AshBufferAccess::new(PipelineStageFlags2::ALL_COMMANDS, AccessFlags2::MEMORY_READ)
			}
			BufferAccess::GeneralWrite => {
				AshBufferAccess::new(PipelineStageFlags2::ALL_COMMANDS, AccessFlags2::MEMORY_WRITE)
			}
			BufferAccess::TransferRead => {
				AshBufferAccess::new(PipelineStageFlags2::TRANSFER, AccessFlags2::TRANSFER_READ)
			}
			BufferAccess::TransferWrite => {
				AshBufferAccess::new(PipelineStageFlags2::TRANSFER, AccessFlags2::TRANSFER_WRITE)
			}
			BufferAccess::ShaderRead => AshBufferAccess::new(
				PipelineStageFlags2::ALL_GRAPHICS | PipelineStageFlags2::COMPUTE_SHADER,
				AccessFlags2::SHADER_STORAGE_READ,
			),
			BufferAccess::ShaderWrite => AshBufferAccess::new(
				PipelineStageFlags2::ALL_GRAPHICS | PipelineStageFlags2::COMPUTE_SHADER,
				AccessFlags2::SHADER_STORAGE_WRITE,
			),
			BufferAccess::ShaderReadWrite => AshBufferAccess::new(
				PipelineStageFlags2::ALL_GRAPHICS | PipelineStageFlags2::COMPUTE_SHADER,
				AccessFlags2::SHADER_STORAGE_READ | AccessFlags2::SHADER_STORAGE_WRITE,
			),
			BufferAccess::HostAccess => AshBufferAccess::new(
				PipelineStageFlags2::HOST,
				AccessFlags2::HOST_READ | AccessFlags2::HOST_WRITE,
			),
			BufferAccess::IndirectCommandRead => {
				AshBufferAccess::new(PipelineStageFlags2::DRAW_INDIRECT, AccessFlags2::INDIRECT_COMMAND_READ)
			}
			BufferAccess::IndexRead => AshBufferAccess::new(PipelineStageFlags2::INDEX_INPUT, AccessFlags2::INDEX_READ),
			BufferAccess::VertexAttributeRead => AshBufferAccess::new(
				PipelineStageFlags2::VERTEX_ATTRIBUTE_INPUT,
				AccessFlags2::VERTEX_ATTRIBUTE_READ,
			),
		}
	}
}

pub struct AshImageAccess {
	pub stage_mask: PipelineStageFlags2,
	pub access_mask: AccessFlags2,
	pub image_layout: ImageLayout,
}

impl AshImageAccess {
	pub const fn new(stage_mask: PipelineStageFlags2, access_mask: AccessFlags2, image_layout: ImageLayout) -> Self {
		Self {
			stage_mask,
			access_mask,
			image_layout,
		}
	}
}

impl ImageAccess {
	pub fn to_ash_image_access(&self) -> AshImageAccess {
		match self {
			ImageAccess::Undefined => AshImageAccess::new(
				PipelineStageFlags2::ALL_COMMANDS,
				AccessFlags2::NONE,
				ImageLayout::UNDEFINED,
			),
			ImageAccess::General => AshImageAccess::new(
				PipelineStageFlags2::ALL_COMMANDS,
				AccessFlags2::MEMORY_READ | AccessFlags2::MEMORY_WRITE,
				ImageLayout::GENERAL,
			),
			ImageAccess::GeneralRead => AshImageAccess::new(
				PipelineStageFlags2::ALL_COMMANDS,
				AccessFlags2::MEMORY_READ,
				ImageLayout::SHADER_READ_ONLY_OPTIMAL,
			),
			ImageAccess::GeneralWrite => AshImageAccess::new(
				PipelineStageFlags2::ALL_COMMANDS,
				AccessFlags2::MEMORY_WRITE,
				ImageLayout::GENERAL,
			),
			ImageAccess::TransferRead => AshImageAccess::new(
				PipelineStageFlags2::TRANSFER,
				AccessFlags2::TRANSFER_READ,
				ImageLayout::TRANSFER_SRC_OPTIMAL,
			),
			ImageAccess::TransferWrite => AshImageAccess::new(
				PipelineStageFlags2::TRANSFER,
				AccessFlags2::TRANSFER_WRITE,
				ImageLayout::TRANSFER_DST_OPTIMAL,
			),
			ImageAccess::StorageRead => AshImageAccess::new(
				PipelineStageFlags2::ALL_GRAPHICS | PipelineStageFlags2::COMPUTE_SHADER,
				AccessFlags2::SHADER_STORAGE_READ,
				ImageLayout::SHADER_READ_ONLY_OPTIMAL,
			),
			ImageAccess::StorageWrite => AshImageAccess::new(
				PipelineStageFlags2::ALL_GRAPHICS | PipelineStageFlags2::COMPUTE_SHADER,
				AccessFlags2::SHADER_STORAGE_WRITE,
				ImageLayout::GENERAL,
			),
			ImageAccess::StorageReadWrite => AshImageAccess::new(
				PipelineStageFlags2::ALL_GRAPHICS | PipelineStageFlags2::COMPUTE_SHADER,
				AccessFlags2::SHADER_STORAGE_READ | AccessFlags2::SHADER_STORAGE_WRITE,
				ImageLayout::GENERAL,
			),
			ImageAccess::SampledRead => AshImageAccess::new(
				PipelineStageFlags2::ALL_GRAPHICS | PipelineStageFlags2::COMPUTE_SHADER,
				AccessFlags2::SHADER_SAMPLED_READ,
				ImageLayout::SHADER_READ_ONLY_OPTIMAL,
			),
			ImageAccess::ColorAttachment => AshImageAccess::new(
				PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
				AccessFlags2::COLOR_ATTACHMENT_READ | AccessFlags2::COLOR_ATTACHMENT_WRITE,
				ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
			),
			ImageAccess::DepthStencilAttachment => AshImageAccess::new(
				PipelineStageFlags2::EARLY_FRAGMENT_TESTS | PipelineStageFlags2::LATE_FRAGMENT_TESTS,
				AccessFlags2::DEPTH_STENCIL_ATTACHMENT_READ | AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
				ImageLayout::DEPTH_ATTACHMENT_STENCIL_READ_ONLY_OPTIMAL,
			),
			ImageAccess::Present => AshImageAccess::new(
				PipelineStageFlags2::ALL_COMMANDS,
				AccessFlags2::NONE,
				ImageLayout::PRESENT_SRC_KHR,
			),
		}
	}
}
