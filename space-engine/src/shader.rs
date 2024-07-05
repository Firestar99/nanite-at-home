//! allow dead code while rust-gpu may still mispile with lod_obj shaders missing
#![allow(dead_code)]

include!(concat!(env!("OUT_DIR"), "/shader_symbols.rs"));
