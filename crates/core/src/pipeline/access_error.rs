use crate::descriptor::{BindlessBufferUsage, BindlessImageUsage};
use crate::pipeline::access_lock::AccessLockError;
use thiserror::Error;

/// An AccessError is a runtime checked error that usually indicates a programming error, and should usually be
/// handled with a `panic!` (or [`Result::unwrap`]).
#[derive(Error)]
#[non_exhaustive]
pub enum AccessError {
	#[error("AccessLockError: {0}")]
	AccessLockError(#[from] AccessLockError),
	#[error("Buffer \"{name}\" with usages {usage:?} is missing usage {missing_usage:?} for this operation")]
	MissingBufferUsage {
		name: String,
		usage: BindlessBufferUsage,
		missing_usage: BindlessBufferUsage,
	},
	#[error("Image \"{name}\" with usages {usage:?} is missing usage {missing_usage:?} for this operation")]
	MissingImageUsage {
		name: String,
		usage: BindlessImageUsage,
		missing_usage: BindlessImageUsage,
	},
}

impl core::fmt::Debug for AccessError {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		core::fmt::Display::fmt(self, f)
	}
}
