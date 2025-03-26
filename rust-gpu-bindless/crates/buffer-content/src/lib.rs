#![no_std]
// otherwise you won't see any warnings on spirv
#![deny(warnings)]

use bytemuck::{AnyBitPattern, Pod};

#[cfg(feature = "glam")]
mod glam;
mod primitive;

pub mod __private {
	pub use bytemuck;
	pub use bytemuck_derive;
	pub use static_assertions;
}

/// Trait for plain structs that need to implement `BufferStruct` and do **not** contain any references, just plain
/// data. Use `#[derive(BufferStructPlain)]` on your struct declaration to implement this trait.
///
/// # Safety
/// The associated type Transfer must be the same on all targets. Writing followed by reading back a value must result
/// in the same value.
pub unsafe trait BufferStructPlain: Copy + Send + Sync + 'static {
	type Transfer: AnyBitPattern + Send + Sync;

	/// # Safety
	/// See [`BufferStructPlain`]
	unsafe fn write(self) -> Self::Transfer;

	/// # Safety
	/// See [`BufferStructPlain`]
	unsafe fn read(from: Self::Transfer) -> Self;
}

/// Trait marking all [`BufferStructPlain`] whose read and write methods are identity. While [`BufferStructPlain`] only
/// requires `t == read(write(t))`, this trait additionally requires `t == read(t) == write(t)`. As this removes the
/// conversion requirement for writing to or reading from a buffer, one can acquire slices from buffers created of these
/// types.
///
/// # Safety
/// The above constraint must hold
pub unsafe trait BufferStructIdentity: Pod + Send + Sync {}

unsafe impl<T: BufferStructIdentity> BufferStructPlain for T {
	type Transfer = Self;

	unsafe fn write(self) -> Self::Transfer {
		self
	}

	unsafe fn read(from: Self::Transfer) -> Self {
		from
	}
}
