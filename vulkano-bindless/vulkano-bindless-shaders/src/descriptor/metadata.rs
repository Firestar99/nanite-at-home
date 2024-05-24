use bytemuck_derive::AnyBitPattern;
use static_assertions::const_assert;

/// Metadata about an execution, like the current frame in flight, to be able to safely upgrade weak pointers.
/// Currently unused.
#[derive(Copy, Clone, AnyBitPattern)]
pub struct Metadata;

/// Reserve 32 bits of push constant for Metadata
pub const METADATA_MAX_SIZE: usize = 32;
const_assert!(core::mem::size_of::<Metadata>() <= METADATA_MAX_SIZE);

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct PushConstant<T> {
	pub t: T,
	pub metadata: Metadata,
}
