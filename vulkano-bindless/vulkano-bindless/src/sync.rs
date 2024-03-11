pub use inner::*;

#[cfg(feature = "loom")]
mod inner {
	use std::sync::TryLockError;

	pub use ::loom::sync::{Arc, Barrier};
	pub use ::loom::sync::{Condvar, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, WaitTimeoutResult};

	pub mod hint {
		pub use loom::hint::{spin_loop, unreachable_unchecked};
	}
	pub mod atomic {
		pub use loom::sync::atomic::{
			fence, AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicIsize, AtomicPtr, AtomicU16, AtomicU32, AtomicU64,
			AtomicU8, AtomicUsize, Ordering,
		};
	}
	pub mod mpsc {
		pub use loom::sync::mpsc::{channel, Receiver, Sender};
	}
	pub mod thread {
		pub use loom::thread::{
			current, panicking, park, spawn, yield_now, AccessError, Builder, JoinHandle, LocalKey, Thread, ThreadId,
		};
	}
	pub mod loom {
		pub use loom::{explore, model, skip_branch, stop_exploring};
	}

	pub mod cell {
		pub use loom::cell::Cell;

		#[derive(Debug)]
		pub struct UnsafeCell<T>(loom::cell::UnsafeCell<T>);

		impl<T> UnsafeCell<T> {
			pub fn new(data: T) -> UnsafeCell<T> {
				UnsafeCell(loom::cell::UnsafeCell::new(data))
			}

			/// SAFETY: same as casting contents to a shared reference
			pub unsafe fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
				unsafe { self.0.with(|t| f(&*t)) }
			}

			/// SAFETY: same as casting contents to a mutable exclusive reference
			pub unsafe fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
				unsafe { self.0.with_mut(|t| f(&mut *t)) }
			}
		}
	}

	pub struct SpinWait;

	#[cfg(feature = "loom")]
	impl SpinWait {
		pub fn new() -> Self {
			Self
		}
		pub fn spin(&mut self) {
			thread::yield_now()
		}
		pub fn spin_no_yield(&mut self) {
			thread::yield_now()
		}
		pub fn reset(&mut self) {}
	}

	pub struct Mutex<T> {
		inner: ::loom::sync::Mutex<T>,
	}

	impl<T> Mutex<T> {
		pub fn new(data: T) -> Self {
			Self {
				inner: ::loom::sync::Mutex::new(data),
			}
		}

		pub fn lock(&self) -> MutexGuard<'_, T> {
			self.inner.lock().unwrap()
		}

		pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
			match self.inner.try_lock() {
				Ok(e) => Some(e),
				Err(TryLockError::WouldBlock) => None,
				Err(TryLockError::Poisoned(e)) => panic!("{}", TryLockError::Poisoned(e)),
			}
		}

		pub fn get_mut(&mut self) -> &mut T {
			self.inner.get_mut().unwrap()
		}

		pub fn into_inner(self) -> T {
			self.inner.into_inner().unwrap()
		}
	}
}

#[cfg(not(feature = "loom"))]
mod inner {
	pub use std::sync::{Arc, Barrier};

	pub use parking_lot::{Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, WaitTimeoutResult};

	pub mod hint {
		pub use std::hint::{spin_loop, unreachable_unchecked};
	}
	pub mod atomic {
		pub use std::sync::atomic::{
			fence, AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicIsize, AtomicPtr, AtomicU16, AtomicU32, AtomicU64,
			AtomicU8, AtomicUsize, Ordering,
		};
	}
	pub mod mpsc {
		pub use std::sync::mpsc::{channel, Receiver, Sender};
	}
	pub mod thread {
		pub use std::thread::{
			current, panicking, park, spawn, yield_now, AccessError, Builder, JoinHandle, LocalKey, Thread, ThreadId,
		};
	}
	pub mod loom {
		pub fn explore() {}
		pub fn stop_exploring() {}
		pub fn skip_branch() {}
		pub fn model<F>(f: F)
		where
			F: Fn() + Sync + Send + 'static,
		{
			f()
		}
	}

	pub mod cell {
		pub use std::cell::Cell;

		#[derive(Debug)]
		pub struct UnsafeCell<T>(std::cell::UnsafeCell<T>);

		impl<T> UnsafeCell<T> {
			pub fn new(data: T) -> UnsafeCell<T> {
				UnsafeCell(std::cell::UnsafeCell::new(data))
			}

			/// SAFETY: same as casting contents to a shared reference
			pub unsafe fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
				unsafe { f(&*self.0.get()) }
			}

			/// SAFETY: same as casting contents to a mutable exclusive reference
			pub unsafe fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
				f(&mut *self.0.get())
			}

			/// not available with loom!
			pub fn get(&self) -> *mut T {
				self.0.get()
			}

			/// not available with loom!
			pub fn get_mut(&mut self) -> &mut T {
				self.0.get_mut()
			}
		}
	}

	pub struct SpinWait(parking_lot_core::SpinWait);

	impl SpinWait {
		pub fn new() -> Self {
			Self(parking_lot_core::SpinWait::new())
		}
		pub fn spin(&mut self) {
			self.0.spin();
		}
		pub fn spin_no_yield(&mut self) {
			self.0.spin_no_yield();
		}
		pub fn reset(&mut self) {
			self.0.reset();
		}
	}
}
