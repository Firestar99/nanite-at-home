use std::cell::UnsafeCell;
use std::marker::PhantomPinned;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::rc::Weak;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32};

use parking_lot::Mutex;

use crate::reinit::Callback;

pub trait ReinitImpl<T> {

}

#[derive(Clone, Debug)]
pub struct Reinit<T>(Pin<Arc<Inner<T>>>);

#[derive(Debug)]
pub struct Inner<T> {
	_pinned: PhantomPinned,

	/// a restart is requested
	restart: AtomicBool,

	/// lock while constructing or destroying t, may also lock hooks while holding this
	construct_lock: Mutex<()>,
	/// ref count for member t
	ref_cnt: AtomicU32,
	t: UnsafeCell<MaybeUninit<T>>,

	/// hooks of everyone wanting to get notified, unordered
	hooks: Mutex<Vec<Weak<dyn Callback<T>>>>,

	details: dyn ReinitImpl<T>,
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