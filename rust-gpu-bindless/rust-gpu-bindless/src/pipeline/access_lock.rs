use num_traits::{FromPrimitive, ToPrimitive};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;
use thiserror::Error;

pub struct AccessLock<A: Copy + FromPrimitive + ToPrimitive> {
	atomic: AtomicU32,
	_phantom: PhantomData<A>,
}

impl<A: Copy + FromPrimitive + ToPrimitive> AccessLock<A> {
	const LOCKED: u32 = !0;
	const SHARED: u32 = !1;

	pub fn new(a: A) -> Self {
		Self {
			atomic: AtomicU32::new(a.to_u32().unwrap()),
			_phantom: PhantomData,
		}
	}

	pub fn try_lock(&self) -> Result<A, AccessLockError> {
		let mut old = self.atomic.load(Relaxed);
		loop {
			match old {
				Self::LOCKED => return Err(AccessLockError::Locked),
				Self::SHARED => return Err(AccessLockError::Shared),
				_ => match self.atomic.compare_exchange_weak(old, Self::LOCKED, Relaxed, Relaxed) {
					Ok(_) => return Ok(A::from_u32(old).unwrap()),
					Err(e) => old = e,
				},
			}
		}
	}

	pub fn unlock(&self, a: A) {
		self.unlock_inner(a.to_u32().unwrap());
	}

	pub fn unlock_to_shared(&self) {
		self.unlock_inner(Self::SHARED);
	}

	#[inline]
	fn unlock_inner(&self, a: u32) {
		self.atomic
			.compare_exchange(Self::LOCKED, a, Relaxed, Relaxed)
			.expect("double unlock");
	}
}

#[derive(Debug, Error)]
pub enum AccessLockError {
	#[error("Resource is locked by another ongoing execution")]
	Locked,
	#[error("Resource is in shared read-only access and cannot be used mutably again")]
	Shared,
}
