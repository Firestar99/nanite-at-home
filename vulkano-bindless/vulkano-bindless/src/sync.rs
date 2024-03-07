pub use inner::*;

#[cfg(feature = "loom")]
mod inner {
	pub use loom::sync::{Arc, Barrier};
	pub use loom::sync::{Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, WaitTimeoutResult};

	pub mod cell {
		pub use loom::cell::{Cell, UnsafeCell};
	}
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

	pub struct SpinWait;

	#[cfg(feature = "loom")]
	impl crate::sync::inner::SpinWait {
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

			pub fn with<R>(&self, f: impl FnOnce(*const T) -> R) -> R {
				f(self.0.get())
			}

			pub fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
				f(self.0.get())
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
