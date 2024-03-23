//! # Frame in flight
//! The Frame in flight system consists out of 3 main components:
//! * `SeedInFlight`: The seed is the configuration of the Frame in flight system and ensures different seeds are not mixed or matched.
//! To construct it you must pass the amount of frames that may be in flight at maximum, so that other systems can allocate enough resources
//! to support that many frames in flight. The maximum may also not exceed the `FRAMES_LIMIT` of 3, see Efficiency for why.
//! * `FrameInFlight`: A Frame in flight is effectively the index of the frame that is currently in flight. It is constructed via the
//! `new()` from a seed and the index that it should represent, which is marked unsafe as one needs to ensure two frames in flight with
//! the same index are never executing at the same time.
//! * `ResourceInFlight`: A `ResourceInFlight` is a resource that is allocated once per frame that may be in flight at the same time.
//! `FrameInFlight` may be used to index into the `ResourceInFlight` to get the resource to be used for that particular frame. The
//! Resources themselves are stored contiguously inside the `ResourceInFlight` type, and not separately on heap.
//!
//! # Efficiency and Safety
//! First each component stores the seed it was constructed with and checks that it is accessed with the seed it was constructed with,
//! so that no mixing of different seeds may occur. This allows the following conditions to always be true:
//!
//! * `SeedInFlight.frames_in_flight <= FRAMES_LIMIT` checked during Seed construction
//! * `FrameInFlight.index < SeedInFlight.frames_in_flight` checked during `FrameInFlight` construction
//!
//! This has two main advantages:
//! * As indices are always checked during construction, indexing of a `ResourceInFlight` with a `FrameInFlight` may happen without
//! checking that the index is in bound.
//! * `ResourceInFlight` has array of its resource instead of having to heap allocate it's data and thus introduce an indirection.
//! This array has a capacity of `FRAMES_LIMIT` and is the main factor in the limit, as raising the limit will inherently increase
//! the size of all `ResourceInFlight` structs. If the seed uses less than `FRAMES_LIMIT` maximum frames in flight, the remaining
//! slots in the array will remain uninitialized, but still contribute to the size of `ResourceInFlight`.
//!

pub use vulkano_bindless_shaders::frame_in_flight::*;

pub mod frame_manager;
pub mod uniform;

pub const FRAMES_LIMIT: u32 = 3;
