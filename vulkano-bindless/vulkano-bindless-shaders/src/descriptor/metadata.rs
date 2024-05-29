use bytemuck_derive::AnyBitPattern;
use static_assertions::const_assert;

/// Metadata about an execution, like the current frame in flight, to be able to safely upgrade weak pointers.
/// Currently unused.
#[derive(Copy, Clone, AnyBitPattern)]
pub struct Metadata;

/// Reserve 32 bits of push constant for Metadata
pub const METADATA_MAX_SIZE: usize = 32;
const_assert!(core::mem::size_of::<Metadata>() <= METADATA_MAX_SIZE);

/// All bindless push constants are this particular struct, with T being the declared push_param.
///
/// Must not derive `DescStruct`, as to [`DescStruct::from_transfer`] Self you'd need the Metadata, which this struct
/// contains. To break the loop, it just stores Metadata flat and params directly as `T::TransferDescStruct`.
#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct PushConstant<T: bytemuck::AnyBitPattern + Send + Sync> {
	pub t: T,
	pub metadata: Metadata,
}
