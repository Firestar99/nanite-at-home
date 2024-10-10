#![no_std]
// otherwise you won't see any warnings on spirv
#![deny(warnings)]

use bytemuck::AnyBitPattern;

mod generic;
#[cfg(feature = "glam")]
mod glam;
mod primitive;

pub use bytemuck;
pub use bytemuck_derive;
pub use static_assertions;

/// Trait for plain structs that need to implement `BufferStruct` and do **not** contain any references, just plain
/// data. Use `#[derive(BufferStructPlain)]` on your struct declaration to implement this trait.
///
/// # blanket impl BufferStruct
/// To actually blanket impl `BufferStruct`, the type must additionally impl [`BufferStructPlainAutoDerive`]. Separating
/// these two traits allows us to specify implementations for both `BufferStruct` and `BufferStructPlain` on types with
/// generics, like an array.
///
/// # Safety
/// The associated type Transfer must be the same on all targets. Passing some value though read and write must result
/// in the same value.
pub unsafe trait BufferStructPlain: Copy + Clone + Sized + Send + Sync {
	type Transfer: AnyBitPattern + Send + Sync;

	/// # Safety
	/// See [`BufferStructPlain`]
	unsafe fn write(self) -> Self::Transfer;

	/// # Safety
	/// See [`BufferStructPlain`]
	unsafe fn read(from: Self::Transfer) -> Self;
}
