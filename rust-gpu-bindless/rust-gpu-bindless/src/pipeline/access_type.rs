#[repr(u8)]
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum BufferAccess {
	Undefined,
	General,
	TransferRead,
	TransferWrite,
	Read,
	/// Write is currently useless, use [`BufferAccess::ReadWrite`] instead
	Write,
	ReadWrite,
	IndirectCommandRead,
	IndexRead,
	VertexAttributeRead,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum ImageAccess {
	Undefined,
	General,
	TransferRead,
	TransferWrite,
	StorageRead,
	/// StorageWrite is currently useless, use [`ImageAccess::StorageReadWrite`] instead
	StorageWrite,
	StorageReadWrite,
	SampledRead,
	ColorAttachment,
	DepthStencilAttachment,
	Present,
}

pub unsafe trait BufferAccessType {
	const BUFFER_ACCESS: BufferAccess;
}

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
access_type!(pub General: BufferAccess::General ImageAccess::General ShaderReadable ShaderWriteable ShaderReadWriteable ShaderSampleable);
access_type!(pub TransferRead: BufferAccess::TransferRead ImageAccess::TransferRead);
access_type!(pub TransferWrite: BufferAccess::TransferWrite ImageAccess::TransferWrite);

access_type!(pub Read: BufferAccess::Read ShaderReadable);
access_type! {
	/// Write is currently useless, use [`ReadWrite`] instead
	pub Write: BufferAccess::Write ShaderWriteable
}
access_type!(pub ReadWrite: BufferAccess::ReadWrite ShaderReadable ShaderWriteable ShaderReadWriteable);
access_type!(pub IndirectCommandRead: BufferAccess::IndirectCommandRead);
access_type!(pub IndexRead: BufferAccess::IndexRead);
access_type!(pub VertexAttributeRead: BufferAccess::VertexAttributeRead);

access_type!(pub StorageRead: ImageAccess::StorageRead ShaderReadable);
access_type! {
	/// StorageWrite is currently useless, use [`StorageReadWrite`] instead
	pub StorageWrite: ImageAccess::StorageWrite ShaderWriteable
}
access_type!(pub StorageReadWrite: ImageAccess::StorageReadWrite ShaderReadable ShaderWriteable ShaderReadWriteable);
access_type!(pub SampledRead: ImageAccess::SampledRead ShaderSampleable);
access_type!(pub ColorAttachment: ImageAccess::ColorAttachment);
access_type!(pub DepthStencilAttachment: ImageAccess::DepthStencilAttachment);
access_type!(pub Present: ImageAccess::Present);
