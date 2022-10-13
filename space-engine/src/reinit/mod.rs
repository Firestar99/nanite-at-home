use std::cell::UnsafeCell;
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomPinned;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, fence};
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};

use parking_lot::Mutex;

pub trait Callback<T: 'static> {
	fn accept(&self, t: ReinitRef<T>);

	fn request_drop(&self);
}

#[repr(transparent)]
pub struct Reinit<T: 'static> {
	ptr: Pin<Arc<Inner<T>>>,
}

pub(super) struct Inner<T: 'static>
{
	_pinned: PhantomPinned,

	/// a restart is requested
	restart: AtomicBool,

	/// lock while constructing or destroying instance, may also lock hooks while holding this
	construct_lock: Mutex<()>,
	/// ref count for member instance
	ref_cnt: AtomicUsize,
	/// instance of T and holding an Arc reference to this to prevent freeing self
	instance: UnsafeCell<MaybeUninit<Instance<T>>>,

	/// hooks of everyone wanting to get notified, unordered
	hooks: Mutex<Vec<Weak<dyn Callback<T>>>>,

	details: Arc<dyn ReinitDetails<T>>,
}

pub trait ReinitDetails<T: 'static>: 'static {
	fn request_construction(&self);
}

pub(super) struct Instance<T: 'static> {
	pub(super) instance: T,
	pub(super) arc: Reinit<T>,
}


// ReinitRef
#[repr(transparent)]
pub struct ReinitRef<T: 'static> {
	inner: NonNull<Inner<T>>,
}

impl<T> ReinitRef<T> {
	#[inline]
	fn new(parent: &Inner<T>) -> Self {
		unsafe { parent.ref_inc() };
		Self { inner: NonNull::from(parent) }
	}

	#[inline]
	fn inner(&self) -> &Inner<T> {
		// SAFETY: parent should exist as ReinitRef ref counts it's existence
		unsafe { self.inner.as_ref() }
	}
}

impl<T> Clone for ReinitRef<T> {
	#[inline]
	fn clone(&self) -> Self {
		ReinitRef::new(self.inner())
	}
}

impl<T> Drop for ReinitRef<T> {
	#[inline]
	fn drop(&mut self) {
		unsafe { self.inner().ref_dec() }
	}
}

impl<T> Deref for ReinitRef<T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		unsafe { &self.inner().ref_get_instance().instance }
	}
}

impl<T: Debug> Debug for ReinitRef<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.deref().fmt(f)
	}
}

impl<T: Display> Display for ReinitRef<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.deref().fmt(f)
	}
}

impl<T> Inner<T> {
	#[inline]
	unsafe fn ref_inc(&self) {
		debug_assert!(self.ref_cnt.load(Relaxed) > 0);
		self.ref_cnt.fetch_add(1, Relaxed);
	}

	#[inline]
	unsafe fn ref_dec(&self) {
		debug_assert!(self.ref_cnt.load(Relaxed) > 0);
		if self.ref_cnt.fetch_sub(1, Relaxed) == 1 {
			self.ref_get_instance().arc.slow_drop();
		}
	}

	#[inline]
	unsafe fn ref_get_instance(&self) -> &Instance<T> {
		debug_assert!(self.ref_cnt.load(Relaxed) > 0);
		(&*self.instance.get()).assume_init_ref()
	}
}


// Reinit impl
impl<T> Reinit<T>
{
	#[inline]
	fn new<D, F>(details: F) -> Reinit<T>
		where
			D: ReinitDetails<T>,
			F: FnOnce(&Weak<Inner<T>>) -> Arc<D>
	{
		let reinit = Reinit::from(Arc::new_cyclic(|weak| Inner {
			_pinned: Default::default(),
			restart: AtomicBool::new(false),
			construct_lock: Mutex::new(()),
			ref_cnt: AtomicUsize::new(0),
			instance: UnsafeCell::new(MaybeUninit::uninit()),
			hooks: Mutex::new(vec![]),
			details: details(weak),
		}));
		reinit.ptr.details.request_construction();
		reinit
	}

	#[inline(always)]
	fn from(arc: Arc<Inner<T>>) -> Self {
		Reinit { ptr: unsafe { Pin::new_unchecked(arc) } }
	}

	fn restart_inc(&self) {}

	fn restart_dec(&self) {}

	#[cold]
	#[inline(never)]
	fn slow_drop(&self) {}

	fn constructed(&self, t: T) {
		// initialize self.instance
		{
			assert_eq!(self.ptr.ref_cnt.load(Relaxed), 0);
			unsafe { &mut *self.ptr.instance.get() }.write(Instance {
				instance: t,
				arc: self.clone(),
			});
			self.ptr.ref_cnt.store(1, Release);
		}

		// call hooks
		{
			let mut hooks = self.ptr.hooks.lock();

			// basically hooks.retain() but with swap_remove() as we don't care about order
			let mut i = 0;
			while i < hooks.len() {
				match hooks[i].upgrade() {
					None => {
						hooks.swap_remove(i);
						// no increment
					}
					Some(h) => {
						h.accept(ReinitRef::new(&*self.ptr));
						i += 1;
					}
				}
			}
		}
	}

	pub fn add_callback<C>(&self, callback: &Arc<C>)
		where C: Callback<T> + 'static
	{
		let mut hooks = self.ptr.hooks.lock();
		hooks.push(Arc::downgrade(callback) as Weak<dyn Callback<T>>);

		if self.ptr.ref_cnt.load(Relaxed) > 0 {
			callback.accept(ReinitRef::new(&*self.ptr));
		}
	}
}

/// #[derive[Clone]) doesn't work as it requires T: Clone which it must not
impl<T> Clone for Reinit<T> {
	fn clone(&self) -> Self {
		Reinit {
			ptr: self.ptr.clone()
		}
	}
}

impl<T> Drop for Inner<T> {
	fn drop(&mut self) {
		// guarantees that self.instance has dropped already
		debug_assert_eq!(self.ref_cnt.load(Relaxed), 0, "self.t must have already dropped at this point");
	}
}


// WeakReinit
#[repr(transparent)]
pub struct WeakReinit<T: 'static> {
	/// no Pin<> wrap: cannot upgrade() otherwise
	ptr: Weak<Inner<T>>,
}

impl<T> WeakReinit<T> {
	fn new(weak: &Weak<Inner<T>>) -> Self {
		Self { ptr: weak.clone() }
	}

	fn upgrade(&self) -> Option<Reinit<T>> {
		self.ptr.upgrade().map(|a| Reinit::from(a))
	}
}


// Dependency
struct Dependency<T: 'static>
{
	reinit: Reinit<T>,
	value: UnsafeCell<Option<ReinitRef<T>>>,
}

impl<T> Dependency<T> {
	fn new(reinit: Reinit<T>) -> Self {
		Self {
			reinit,
			value: UnsafeCell::new(None),
		}
	}

	#[inline]
	fn value_set(&self, t: ReinitRef<T>) {
		let cell = unsafe { &mut *self.value.get() };
		debug_assert!(matches!(cell, None));
		*cell = Some(t);
	}

	#[inline]
	fn value_clear(&self) {
		let cell = unsafe { &mut *self.value.get() };
		debug_assert!(matches!(cell, Some(..)));
		*cell = None;
	}

	#[inline]
	fn value_get(&self) -> &ReinitRef<T> {
		let cell = unsafe { &mut *self.value.get() };
		debug_assert!(matches!(cell, Some(..)));
		unsafe { cell.as_ref().unwrap_unchecked() }
	}
}


// Reinit0
struct Reinit0<T: 'static, F>
	where
		F: Fn() -> T + 'static
{
	parent: WeakReinit<T>,
	constructor: F,
}

impl<T: 'static> Reinit<T>
{
	pub fn new0<F>(constructor: F) -> Reinit<T>
		where
			F: Fn() -> T + 'static
	{
		Reinit::new(|weak| Arc::new(Reinit0 {
			parent: WeakReinit::new(weak),
			constructor,
		}))
	}
}

impl<T: 'static, F> ReinitDetails<T> for Reinit0<T, F>
	where
		F: Fn() -> T + 'static
{
	fn request_construction(&self) {
		if let Some(parent) = &self.parent.upgrade() {
			parent.constructed((self.constructor)())
		}
	}
}


// Reinit1
struct Reinit1<T: 'static, F, A: 'static>
	where
		F: Fn(ReinitRef<A>) -> T + 'static
{
	parent: WeakReinit<T>,
	constructor: F,
	countdown: AtomicU32,
	a: Dependency<A>,
}

impl<T: 'static> Reinit<T>
{
	pub fn new1<F, A: 'static>(a: Reinit<A>, constructor: F) -> Reinit<T>
		where
			F: Fn(ReinitRef<A>) -> T + 'static
	{
		Reinit::new(|weak| {
			let this = Arc::new(Reinit1 {
				parent: WeakReinit::new(weak),
				a: Dependency::new(a),
				countdown: AtomicU32::new(1 + 1),
				constructor,
			});
			this.a.reinit.add_callback(&this);
			this
		})
	}
}

impl<T: 'static, F, A: 'static> Reinit1<T, F, A>
	where
		F: Fn(ReinitRef<A>) -> T + 'static
{
	fn construct_countdown(&self) {
		// TODO proper construct_lock usage
		if self.countdown.fetch_sub(1, Release) == 1 {
			fence(Acquire);
			if let Some(parent) = self.parent.upgrade() {
				parent.constructed((self.constructor)(self.a.value_get().clone()));
			}
		}
	}
}

impl<T: 'static, F, A: 'static> ReinitDetails<T> for Reinit1<T, F, A>
	where
		F: Fn(ReinitRef<A>) -> T + 'static
{
	fn request_construction(&self) {
		self.construct_countdown()
	}
}

impl<T: 'static, F, A: 'static> Callback<A> for Reinit1<T, F, A>
	where
		F: Fn(ReinitRef<A>) -> T + 'static
{
	fn accept(&self, t: ReinitRef<A>) {
		self.a.value_set(t);
		self.construct_countdown();
	}

	fn request_drop(&self) {
		self.a.value_clear();
		// TODO drop instance
	}
}
