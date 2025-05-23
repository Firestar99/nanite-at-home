use static_assertions::const_assert;

pub mod indices;
pub mod lod_level_bitmask;
pub mod mesh;
pub mod offset;
pub mod vertex;

pub const MESHLET_MODEL_VERTICES_BITS: u32 = 24;
pub const MESHLET_MODEL_MAX_VERTICES: u32 = 1 << MESHLET_MODEL_VERTICES_BITS;

pub const MESHLET_INDICES_BITS: u32 = 6;
pub const MESHLET_MAX_VERTICES: u32 = 64;
const_assert!(MESHLET_MAX_VERTICES <= 1 << MESHLET_INDICES_BITS);

pub const MESHLET_TRIANGLES_BITS: u32 = 7;
pub const MESHLET_MAX_TRIANGLES: u32 = 124;
const_assert!(MESHLET_MAX_TRIANGLES <= 1 << MESHLET_TRIANGLES_BITS);
