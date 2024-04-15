pub use inner::*;

mod common {
	pub mod loom {
		use crate::sync::thread::spawn;
		use crate::sync::thread::JoinHandle;

		pub fn launch_loom_threads(iter: impl Iterator<Item = impl FnOnce() + Send + 'static>) {
			let (first, _) = launch_loom_threads_inner(iter);
			first();
		}

		pub fn launch_loom_threads_and_wait<T: Send + 'static>(
			iter: impl Iterator<Item = impl FnOnce() -> T + Send + 'static>,
		) -> Vec<T> {
			let (first, joins) = launch_loom_threads_inner(iter);
			[first()]
				.into_iter()
				.chain(joins.into_iter().map(|j| j.join().unwrap()))
				.collect()
		}

		fn launch_loom_threads_inner<F, T>(iter: impl Iterator<Item = F>) -> (F, Vec<JoinHandle<T>>)
		where
			T: Send + 'static,
			F: FnOnce() -> T + Send + 'static,
		{
			let mut vec = iter.collect::<Vec<_>>().into_iter();
			let first = vec.next().unwrap();
			let joins = vec.map(|x| spawn(x)).collect::<Vec<_>>();
			(first, joins)
		}
	}
}

#[cfg(feature = "loom_tests")]
mod inner {
	use std::cell::Cell;
	use std::sync::TryLockError;

	pub use ::loom::sync::{Arc, Barrier};
	pub use ::loom::sync::{Condvar, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, WaitTimeoutResult};
	pub use crossbeam_utils::CachePadded;

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
		pub use loom::model::Builder;
		pub use loom::{explore, model, skip_branch, stop_exploring};

		pub use crate::sync::common::loom::*;

		pub fn model_builder<B, F>(_b: B, f: F)
		where
			B: FnOnce(&mut Builder),
			F: Fn() + Sync + Send + 'static,
		{
			let mut builder = Builder::new();
			_b(&mut builder);
			builder.check(f)
		}
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

	pub struct Backoff(Cell<u32>);

	#[cfg(feature = "loom_tests")]
	impl Backoff {
		// this does almost nothing
		pub const KILL_BRANCH_LIMIT: u32 = 6;
		pub fn new() -> Self {
			Self(Cell::new(0))
		}
		pub fn spin(&self) {
			// failed cas should just repeat
		}
		pub fn snooze(&mut self) {
			if self.0.get() < Self::KILL_BRANCH_LIMIT {
				thread::yield_now();
			} else {
				loom::skip_branch();
			}

			if self.0.get() <= Self::KILL_BRANCH_LIMIT {
				self.0.set(self.0.get() + 1);
			}
		}
		pub fn reset(&mut self) {
			self.0.set(0);
		}
		pub fn is_completed(&self) -> bool {
			self.0.get() > Self::KILL_BRANCH_LIMIT
		}
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

#[cfg(not(feature = "loom_tests"))]
mod inner {
	pub use crossbeam_utils::CachePadded;
	pub use parking_lot::{Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, WaitTimeoutResult};
	pub use std::sync::{Arc, Barrier};

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
		pub use loom::model::Builder;

		pub use crate::sync::common::loom::*;

		pub fn explore() {}
		pub fn stop_exploring() {}
		pub fn skip_branch() {}
		pub fn model<F>(f: F)
		where
			F: Fn() + Sync + Send + 'static,
		{
			f()
		}

		pub fn model_builder<B, F>(_b: B, f: F)
		where
			B: FnOnce(&mut Builder),
			F: Fn() + Sync + Send + 'static,
		{
			f();
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

	pub struct Backoff(crossbeam_utils::Backoff);

	impl Backoff {
		pub fn new() -> Self {
			Self(crossbeam_utils::Backoff::new())
		}
		pub fn spin(&mut self) {
			self.0.spin();
		}
		pub fn snooze(&mut self) {
			self.0.snooze();
		}
		pub fn reset(&mut self) {
			self.0.reset();
		}
		pub fn is_completed(&self) -> bool {
			self.0.is_completed()
		}
	}
}
