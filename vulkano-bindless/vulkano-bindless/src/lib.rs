#![cfg_attr(feature = "loom_tests", feature(arbitrary_self_types))]

//! An attempt at making a safe fully-bindless system for Vulkano.
//!
//! # Safety
//! * One must use [`FrameManager`] to ensure the bindless descriptor set is properly updated and resources are kept
//! valid during the execution of commands.
//! * Additionally, it is forbidden to use *any* of [`GpuFuture`]'s flush methods, like [`GpuFuture::flush`],
//! [`GpuFuture::then_signal_semaphore_and_flush`] or [`GpuFuture::then_signal_fence_and_flush`]. Instead, one
//! should use the respective methods on [`FrameManager`]'s [`Frame`], which ensures proper flushing of the bindless
//! descriptor set.
//!
//! [`FrameManager`]: frame_manager::FrameManager
//! [`Frame`]: frame_manager::Frame
//! [`GpuFuture`]: vulkano::sync::GpuFuture
//! [`GpuFuture::flush`]: vulkano::sync::GpuFuture::flush
//! [`GpuFuture::then_signal_semaphore_and_flush`]: vulkano::sync::GpuFuture::then_signal_semaphore_and_flush
//! [`GpuFuture::then_signal_fence_and_flush`]: vulkano::sync::GpuFuture::then_signal_fence_and_flush
pub mod descriptor;
pub mod frame_in_flight;
pub mod frame_manager;
pub mod rc_slots;
pub mod required_features;
pub mod sync;

pub use vulkano_bindless_shaders::{spirv, spirv_std, Image};
