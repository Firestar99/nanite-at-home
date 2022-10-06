use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::atomic::Ordering::Relaxed;

use parking_lot::Mutex;

// Callback
pub trait Callback<T> {
	fn accept(&self, t: ReinitRef<T>);

	fn request_drop(&self);
}


// ReinitImpl specialization
pub(super) trait ReinitImpl {
	fn request_drop(&self);
}

const REINIT_IMPL_NULL: ReinitImplNull = ReinitImplNull {};

struct ReinitImplNull {}

impl ReinitImpl for ReinitImplNull {
	fn request_drop(&self) {
		panic!()
	}
}


// Reinit
pub(super) struct Reinit<T> {
	specialization: *const dyn ReinitImpl,

	/// a restart is requested
	restart: AtomicBool,

	/// lock while constructing or destroying t, may also lock hooks while holding this
	construct_lock: Mutex<()>,
	/// ref count for member t
	ref_cnt: AtomicU32,
	t: UnsafeCell<MaybeUninit<T>>,

	/// hooks of everyone wanting to get notified, unordered
	hooks: Mutex<Vec<Weak<dyn Callback<T>>>>,
}

impl<T> Reinit<T> {
	pub(super) fn new() -> Self {
		Self {
			specialization: &REINIT_IMPL_NULL,
			restart: AtomicBool::new(false),
			construct_lock: Mutex::new(()),
			ref_cnt: AtomicU32::new(0),
			t: UnsafeCell::new(MaybeUninit::uninit()),
			hooks: Mutex::new(vec![]),
		}
	}

	pub(super) fn init(&mut self, specialization: *const dyn ReinitImpl) {
		self.specialization = specialization;
	}

	fn restart_inc(&self) {}

	fn restart_dec(&self) {}

	#[inline]
	fn ref_inc(&self) {
		self.ref_cnt.fetch_add(1, Relaxed);
	}

	#[inline]
	fn ref_dec(&self) {
		if self.ref_cnt.fetch_sub(1, Relaxed) == 1 {
			self.slow_drop();
		}
	}

	#[cold]
	#[inline(never)]
	fn slow_drop(&self) {}

	pub(super) unsafe fn constructed(&self) {
		let mut hooks = self.hooks.lock();

		// basically hooks.retain() but with swap_remove() as we don't care about order
		let mut i = 0;
		while i < hooks.len() {
			match hooks[i].upgrade() {
				None => {
					hooks.swap_remove(i);
					// no increment
				}
				Some(h) => {
					h.accept(ReinitRef::new(self));
					i += 1;
				}
			}
		}
	}

	pub fn add_callback<C>(&self, callback: &Arc<C>)
		where C: Callback<T> + 'static
	{
		let mut hooks = self.hooks.lock();
		let arc = Arc::downgrade(callback);
		let weak = arc as Weak<dyn Callback<T>>;
		hooks.push(weak);
		// drop(weak);
		// println!("{:?}", weak);

		if self.ref_cnt.load(Relaxed) > 0 {
			callback.accept(ReinitRef::new(self));
		}
	}
}

impl<T> Drop for Reinit<T> {
	fn drop(&mut self) {
		// self.t must have already dropped at this point
		assert_eq!(self.ref_cnt.load(Relaxed), 0);
	}
}


// ReinitRef
#[repr(transparent)]
pub struct ReinitRef<'a, T> {
	parent: &'a Reinit<T>,
}

impl<'a, T> ReinitRef<'a, T> {
	#[inline]
	fn new(parent: &'a Reinit<T>) -> Self {
		parent.ref_inc();
		Self {
			parent
		}
	}
}

impl<'a, T> Clone for ReinitRef<'a, T> {
	#[inline]
	fn clone(&self) -> Self {
		ReinitRef::new(self.parent)
	}
}

impl<'a, T> Drop for ReinitRef<'a, T> {
	#[inline]
	fn drop(&mut self) {
		self.parent.ref_dec();
	}
}

impl<'a, T> Deref for ReinitRef<'a, T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		unsafe {
			// SAFETY: t should exist as ReinitRef ref counts for t
			(&*self.parent.t.get()).assume_init_ref()
		}
	}
}
