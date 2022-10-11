use std::cell::UnsafeCell;
use std::marker::PhantomPinned;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::atomic::Ordering::{Relaxed, Release};

use parking_lot::Mutex;

pub trait Callback<T> {
	fn accept(&self, t: ReinitRef<T>);

	fn request_drop(&self);
}

#[repr(transparent)]
pub struct Reinit<T> {
	ptr: Pin<Arc<Inner<T>>>,
}

pub(super) struct Inner<T>
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

	details: Box<dyn ReinitDetails<T>>,
}

pub trait ReinitDetails<T>: 'static {
	fn init(&self, inner: &Reinit<T>);

	fn request_construction(&self, inner: &Reinit<T>);
}

pub(super) struct Instance<T> {
	pub(super) instance: T,
	pub(super) arc: Reinit<T>,
}


// ReinitRef
#[repr(transparent)]
pub struct ReinitRef<T> {
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

impl<T> Inner<T> {
	#[inline]
	unsafe fn ref_inc(&self) {
		assert!(self.ref_cnt.load(Relaxed) > 0);
		self.ref_cnt.fetch_add(1, Relaxed);
	}

	#[inline]
	unsafe fn ref_dec(&self) {
		assert!(self.ref_cnt.load(Relaxed) > 0);
		if self.ref_cnt.fetch_sub(1, Relaxed) == 1 {
			self.ref_get_instance().arc.slow_drop();
		}
	}

	unsafe fn ref_get_instance(&self) -> &Instance<T> {
		assert!(self.ref_cnt.load(Relaxed) > 0);
		(&*self.instance.get()).assume_init_ref()
	}
}


// Reinit impl
/// #[derive[Clone]) doesn't work as it requires T: Clone which it must not
impl<T> Clone for Reinit<T> {
	fn clone(&self) -> Self {
		Reinit {
			ptr: self.ptr.clone()
		}
	}
}

impl<T> Reinit<T>
{
	fn new<D: ReinitDetails<T>>(details: D) -> Reinit<T> {
		let reinit = Reinit {
			ptr: Arc::pin(Inner {
				_pinned: Default::default(),
				restart: AtomicBool::new(false),
				construct_lock: Mutex::new(()),
				ref_cnt: AtomicUsize::new(0),
				instance: UnsafeCell::new(MaybeUninit::uninit()),
				hooks: Mutex::new(vec![]),
				details: Box::new(details),
			})
		};
		reinit.ptr.details.init(&reinit);
		reinit.ptr.details.request_construction(&reinit);
		reinit
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

impl<T> Drop for Inner<T> {
	fn drop(&mut self) {
		// guarantees that self.instance has dropped already
		assert_eq!(self.ref_cnt.load(Relaxed), 0, "self.t must have already dropped at this point");
	}
}


// Reinit0Impl
struct Reinit0Impl<T, F>
	where
		T: 'static,
		F: Fn() -> T + 'static
{
	constructor: F,
}

impl<T, F> Reinit0Impl<T, F>
	where
		T: 'static,
		F: Fn() -> T + 'static
{
	fn new0(constructor: F) -> Reinit<T> {
		Reinit::new(Self {
			constructor
		})
	}
}

impl<T, F> ReinitDetails<T> for Reinit0Impl<T, F>
	where
		T: 'static,
		F: Fn() -> T + 'static
{
	fn init(&self, _inner: &Reinit<T>) {}

	fn request_construction(&self, inner: &Reinit<T>) {
		inner.constructed((self.constructor)())
	}
}


// Dependency
struct Dependency<T>
	where
		T: 'static
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

	fn value_set(&self, t: ReinitRef<T>) {
		unsafe {
			assert!((&*self.value.get()) == None);
			self.value.get().write(Some(t));
		}
	}
}


// Reinit1Impl
struct Reinit1Impl<T, F, A>
	where
		A: 'static,
		T: 'static,
		F: Fn(ReinitRef<A>) -> T + 'static
{
	constructor: F,
	a: Dependency<A>,
}

impl<T, F, A> Reinit1Impl<T, F, A>
	where
		A: 'static,
		T: 'static,
		F: Fn(ReinitRef<A>) -> T + 'static
{
	fn new1(a: Reinit<A>, constructor: F) -> Reinit<T> {
		Reinit::new(Self {
			a: Dependency::new(a),
			constructor,
		})
	}
}

impl<T, F, A> ReinitDetails<T> for Reinit1Impl<T, F, A>
	where
		A: 'static,
		T: 'static,
		F: Fn(ReinitRef<A>) -> T + 'static
{
	fn init(&self, inner: &Reinit<T>) {
		self.a.reinit.add_callback(self);
	}

	fn request_construction(&self, inner: &Reinit<T>) {
		inner.constructed((self.constructor)(self.a))
	}
}

impl<T, F, A> Callback<A> for Reinit1Impl<T, F, A>
	where
		A: 'static,
		T: 'static,
		F: Fn(ReinitRef<A>) -> T + 'static
{
	fn accept(&self, t: ReinitRef<A>) {
		self.a.value = t;
	}

	fn request_drop(&self) {
		todo!()
	}
}
