// All AccessType traits are unsafe and don't need a safety section
#![allow(clippy::missing_safety_doc)]

use crate::descriptor::{BindlessBufferUsage, BindlessImageUsage};
use num_derive::{FromPrimitive, ToPrimitive};

#[repr(u8)]
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, FromPrimitive, ToPrimitive)]
pub enum BufferAccess {
	Undefined,
	General,
	TransferRead,
	TransferWrite,
	ShaderRead,
	/// Write is currently useless, use [`BufferAccess::ShaderReadWrite`] instead
	ShaderWrite,
	ShaderReadWrite,
	GeneralRead,
	GeneralWrite,
	HostAccess,
	IndirectCommandRead,
	IndexRead,
	VertexAttributeRead,
}

impl BufferAccess {
	/// Returns the required [`BindlessBufferUsage`] flags to transition to this state.
	///
	/// Note that this just verifies being allowed to transition to that state, and does not say anything about how you
	/// can use the buffer once in this state. For example, you can always transition a buffer to general, and while
	/// general allows you to create a [`TransientDesc`] with read access to your buffer, the function creating said
	/// [`TransientDesc`] still needs to runtime check that your buffer has [`BindlessBufferUsage::STORAGE_BUFFER`].
	pub fn required_buffer_usage(&self) -> BindlessBufferUsage {
		match self {
			BufferAccess::Undefined => BindlessBufferUsage::empty(),
			BufferAccess::General => BindlessBufferUsage::empty(),
			BufferAccess::TransferRead => BindlessBufferUsage::TRANSFER_SRC,
			BufferAccess::TransferWrite => BindlessBufferUsage::TRANSFER_DST,
			BufferAccess::ShaderRead => BindlessBufferUsage::STORAGE_BUFFER,
			BufferAccess::ShaderWrite => BindlessBufferUsage::STORAGE_BUFFER,
			BufferAccess::ShaderReadWrite => BindlessBufferUsage::STORAGE_BUFFER,
			BufferAccess::GeneralRead => BindlessBufferUsage::empty(),
			BufferAccess::GeneralWrite => BindlessBufferUsage::empty(),
			BufferAccess::HostAccess => BindlessBufferUsage::empty(),
			BufferAccess::IndirectCommandRead => BindlessBufferUsage::INDIRECT_BUFFER,
			BufferAccess::IndexRead => BindlessBufferUsage::INDEX_BUFFER,
			BufferAccess::VertexAttributeRead => BindlessBufferUsage::VERTEX_BUFFER,
		}
	}
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, FromPrimitive, ToPrimitive)]
pub enum ImageAccess {
	Undefined,
	General,
	TransferRead,
	TransferWrite,
	/// StorageRead is currently useless, use [`SampledRead`] or [`StorageReadWrite`] instead
	StorageRead,
	/// StorageWrite is currently useless, use [`ImageAccess::StorageReadWrite`] instead
	StorageWrite,
	StorageReadWrite,
	GeneralRead,
	GeneralWrite,
	SampledRead,
	ColorAttachment,
	DepthStencilAttachment,
	Present,
}

impl ImageAccess {
	/// Returns the required [`BindlessImageUsage`] flags to transition to this state.
	///
	/// Note that this just verifies being allowed to transition to that state, and does not say anything about how you
	/// can use the image once in this state. For example, you can always transition an image to general, and while
	/// general allows you to create a [`TransientDesc`] with read access to your image, the function creating said
	/// [`TransientDesc`] still needs to runtime check that your image has [`BindlessImageUsage::SAMPLED`] or
	/// [`BindlessImageUsage::STORAGE`], depending on what is required.
	pub fn required_image_usage(&self) -> BindlessImageUsage {
		match self {
			ImageAccess::Undefined => BindlessImageUsage::empty(),
			ImageAccess::General => BindlessImageUsage::empty(),
			ImageAccess::TransferRead => BindlessImageUsage::TRANSFER_SRC,
			ImageAccess::TransferWrite => BindlessImageUsage::TRANSFER_DST,
			ImageAccess::StorageRead => BindlessImageUsage::STORAGE,
			ImageAccess::StorageWrite => BindlessImageUsage::STORAGE,
			ImageAccess::StorageReadWrite => BindlessImageUsage::STORAGE,
			ImageAccess::GeneralRead => BindlessImageUsage::empty(),
			ImageAccess::GeneralWrite => BindlessImageUsage::empty(),
			ImageAccess::SampledRead => BindlessImageUsage::SAMPLED,
			ImageAccess::ColorAttachment => BindlessImageUsage::COLOR_ATTACHMENT,
			ImageAccess::DepthStencilAttachment => BindlessImageUsage::DEPTH_STENCIL_ATTACHMENT,
			ImageAccess::Present => BindlessImageUsage::SWAPCHAIN,
		}
	}
}

/// AccessType of a Buffer
pub unsafe trait BufferAccessType {
	const BUFFER_ACCESS: BufferAccess;
}

/// AccessType of an Image
pub unsafe trait ImageAccessType {
	const IMAGE_ACCESS: ImageAccess;
}

/// AccessType that allows a shader to read from a buffer or storage image
pub unsafe trait ShaderReadable {}

/// AccessType that allows a shader to write to a buffer or storage image
pub unsafe trait ShaderWriteable {}

/// AccessType that allows a shader to read from and write to a buffer or storage image
pub unsafe trait ShaderReadWriteable: ShaderReadable + ShaderWriteable {}

/// AccessType that allows a shader to sample a sampled image
pub unsafe trait ShaderSampleable {}

/// AccessType that allows a transfer operation to read from it
pub unsafe trait TransferReadable {}

/// AccessType that allows a transfer operation to read from it
pub unsafe trait TransferWriteable {}

/// AccessType that allows this buffer to be read as an index buffer
pub unsafe trait IndexReadable {}

/// AccessType that allows this buffer to be read as an index buffer
pub unsafe trait IndirectCommandReadable {}

macro_rules! access_type {
    (@inner $name:ident: BufferAccess::$access:ident $($tt:tt)*) => {
		unsafe impl BufferAccessType for $name {
			const BUFFER_ACCESS: BufferAccess = BufferAccess::$access;
		}
		access_type!(@inner $name: $($tt)*);
	};
    (@inner $name:ident: ImageAccess::$access:ident $($tt:tt)*) => {
		unsafe impl ImageAccessType for $name {
			const IMAGE_ACCESS: ImageAccess = ImageAccess::$access;
		}
		access_type!(@inner $name: $($tt)*);
	};
    (@inner $name:ident: $impltrait:ident $($tt:tt)*) => {
		unsafe impl $impltrait for $name {}
		access_type!(@inner $name: $($tt)*);
	};
    (@inner $name:ident:) => {};
    ($(#[$attrib:meta])* $vis:vis $name:ident: $($tt:tt)*) => {
		$(#[$attrib])*
		$vis struct $name;
		access_type!(@inner $name: $($tt)*);
	};
}

access_type!(pub Undefined: BufferAccess::Undefined ImageAccess::Undefined);
access_type!(pub General: BufferAccess::General ImageAccess::General ShaderReadable ShaderWriteable ShaderReadWriteable
	ShaderSampleable TransferReadable TransferWriteable IndexReadable IndirectCommandReadable);
access_type!(pub GeneralRead: BufferAccess::GeneralRead ImageAccess::GeneralRead ShaderReadable ShaderSampleable
	TransferReadable IndexReadable IndirectCommandReadable);
access_type!(pub GeneralWrite: BufferAccess::GeneralWrite ImageAccess::GeneralWrite ShaderWriteable TransferWriteable);
access_type!(pub TransferRead: BufferAccess::TransferRead ImageAccess::TransferRead TransferReadable);
access_type!(pub TransferWrite: BufferAccess::TransferWrite ImageAccess::TransferWrite TransferWriteable);

access_type!(pub ShaderRead: BufferAccess::ShaderRead ShaderReadable);
access_type! {
	/// Write is currently useless, use [`ShaderReadWrite`] instead
	pub ShaderWrite: BufferAccess::ShaderWrite ShaderWriteable
}
access_type!(pub ShaderReadWrite: BufferAccess::ShaderReadWrite ShaderReadable ShaderWriteable ShaderReadWriteable);
access_type!(pub HostAccess: BufferAccess::HostAccess);
access_type!(pub IndirectCommandRead: BufferAccess::IndirectCommandRead IndirectCommandReadable);
access_type!(pub IndexRead: BufferAccess::IndexRead IndexReadable);
access_type!(pub VertexAttributeRead: BufferAccess::VertexAttributeRead);

access_type! {
	/// StorageRead is currently useless, use [`SampledRead`] or [`StorageReadWrite`] instead
	pub StorageRead: ImageAccess::StorageRead ShaderReadable
}
access_type! {
	/// StorageWrite is currently useless, use [`StorageReadWrite`] instead
	pub StorageWrite: ImageAccess::StorageWrite ShaderWriteable
}
access_type!(pub StorageReadWrite: ImageAccess::StorageReadWrite ShaderReadable ShaderWriteable ShaderReadWriteable);
access_type!(pub SampledRead: ImageAccess::SampledRead ShaderSampleable);
access_type!(pub ColorAttachment: ImageAccess::ColorAttachment);
access_type!(pub DepthStencilAttachment: ImageAccess::DepthStencilAttachment);
access_type!(pub Present: ImageAccess::Present);
