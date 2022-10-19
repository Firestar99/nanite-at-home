use std::cell::{Cell, UnsafeCell};
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomPinned;
use std::mem;
use std::mem::{MaybeUninit, transmute};
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};
use std::sync::atomic::Ordering::{Relaxed, Release};

use parking_lot::{Mutex, RawMutex, RawThreadId, ReentrantMutex};
use parking_lot::lock_api::ReentrantMutexGuard;

pub trait Callback<T: 'static> {
	fn accept(&self, t: ReinitRef<T>);

	fn request_drop(&self);
}

#[repr(transparent)]
pub struct Reinit<T: 'static> {
	ptr: Pin<Arc<Inner<T>>>,
}

struct Inner<T: 'static>
{
	_pinned: PhantomPinned,

	// members contributing to next state computation
	/// countdown to construction, used to count down construction of dependent Reinits, used for restarting by increasing it by one
	countdown: AtomicU32,
	/// true if self should restart, restart happens when self is in state Initialized
	queued_restart: AtomicBool,

	// state
	/// lock while constructing or destroying instance, may also lock hooks while holding this
	state_lock: ReentrantMutex<Cell<State>>,
	/// ref count for member instance
	ref_cnt: AtomicUsize,
	/// instance of T and holding an Arc reference to this to prevent freeing self
	instance: UnsafeCell<MaybeUninit<Instance<T>>>,

	/// hooks of everyone wanting to get notified, unordered
	hooks: Mutex<Vec<Hook<T>>>,

	details: Arc<dyn ReinitDetails<T>>,
}

#[repr(u8)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum State {
	Uninitialized,
	Constructing,
	Initialized,
	Destructing,
}

struct Instance<T: 'static> {
	instance: T,
	arc: Reinit<T>,
}

trait ReinitDetails<T: 'static>: 'static {
	fn request_construction(&self, parent: &Reinit<T>);
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
			(&*self.instance.get()).assume_init_ref().arc.slow_drop();
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
	fn new<D, F, I>(initial_countdown: u32, details_construct: F, details_init: I) -> Reinit<T>
		where
			D: ReinitDetails<T>,
			F: FnOnce(&Weak<Inner<T>>) -> Arc<D>,
			I: FnOnce(&Arc<D>),
	{
		let mut details_arc = None;
		let this = Reinit::from(Arc::new_cyclic(|weak| {
			let details = details_construct(weak);
			details_arc = Some(details.clone());
			Inner {
				_pinned: Default::default(),
				countdown: AtomicU32::new(initial_countdown + 1),
				queued_restart: AtomicBool::new(false),
				state_lock: ReentrantMutex::new(Cell::new(State::Uninitialized)),
				ref_cnt: AtomicUsize::new(0),
				instance: UnsafeCell::new(MaybeUninit::uninit()),
				hooks: Mutex::new(vec![]),
				details,
			}
		}));
		details_init(details_arc.as_ref().unwrap());
		this.construct_countdown();
		this
	}

	#[inline(always)]
	fn from(arc: Arc<Inner<T>>) -> Self {
		Reinit { ptr: unsafe { Pin::new_unchecked(arc) } }
	}

	fn restart(&self) {
		// TODO check this later
		// TODO restart called during construct/destruct must be handled correctly
		// self.construct_countup();
		// self.construct_countdown();
		if self.ptr.queued_restart.compare_exchange(false, true, Relaxed, Relaxed).is_ok() {
			self.check_state();
		}
	}

	#[inline]
	fn construct_countdown(&self) {
		if self.ptr.countdown.fetch_sub(1, Release) == 1 {
			self.check_state();
		}
	}

	#[inline]
	fn construct_countup(&self) {
		if self.ptr.countdown.fetch_add(1, Relaxed) == 0 {
			self.check_state();
		}
	}

	fn check_state(&self) {
		// lock is held for the entire method
		let guard = self.ptr.state_lock.lock();
		if matches!(guard.get(), State::Constructing | State::Destructing) {
			// do nothing, wait for construction / destruction to finish
			return;
		}

		self.check_state_internal(&guard);
	}

	fn check_state_internal(&self, guard: &ReentrantMutexGuard<RawMutex, RawThreadId, Cell<State>>) {
		// figure out target state
		let is_init = guard.get() == State::Initialized;
		let mut should_be_init = self.ptr.countdown.load(Relaxed) == 0;
		if should_be_init && self.ptr.queued_restart.compare_exchange(true, false, Relaxed, Relaxed).is_ok() {
			should_be_init = false;
		}
		if is_init == should_be_init {
			// already in correct state
			return;
		}

		// not in correct state
		if should_be_init {
			assert!(guard.get() == State::Uninitialized);
			guard.set(State::Constructing);

			self.ptr.details.request_construction(self);
		} else {
			assert!(guard.get() == State::Initialized);
			guard.set(State::Destructing);

			// SAFETY: decrement initial ref count owned by ourselves
			unsafe {
				self.ptr.ref_dec();
			}

			self.call_callbacks(|h| h.request_drop());
		}
	}

	fn constructed(&self, t: T) {
		// lock is held for the entire method
		let guard = self.ptr.state_lock.lock();

		// change state
		assert!(guard.get() == State::Constructing);
		guard.set(State::Initialized);

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
		self.call_callbacks(|h| h.accept(ReinitRef::new(&*self.ptr)));

		// TODO do not call hooks if we should destruct immediately
		self.check_state_internal(&guard);
	}

	#[cold]
	#[inline(never)]
	fn slow_drop(&self) {
		// lock is held for the entire method
		let guard = self.ptr.state_lock.lock();

		// change state
		assert!(guard.get() == State::Destructing);
		guard.set(State::Uninitialized);

		// drop self.instance as noone is referencing it anymore
		unsafe {
			(&mut *self.ptr.instance.get()).assume_init_drop()
		};

		self.check_state_internal(&guard);
	}

	fn call_callbacks<F>(&self, f: F)
		where
			F: Fn(&Hook<T>) -> bool
	{
		let mut hooks = self.ptr.hooks.lock();
		// basically hooks.retain() but with swap_remove() as we don't care about order
		let mut i = 0;
		while i < hooks.len() {
			if f(&hooks[i]) {
				i += 1;
			} else {
				hooks.swap_remove(i);
			}
		}
	}

	pub fn add_callback<C>(&self, arc: &Arc<C>, accept: fn(Arc<C>, ReinitRef<T>), request_drop: fn(Arc<C>))
	{
		let mut hooks = self.ptr.hooks.lock();
		let hook = Hook::new(arc, accept, request_drop);
		if self.ptr.ref_cnt.load(Relaxed) > 0 {
			hook.accept(ReinitRef::new(&*self.ptr));
		}
		hooks.push(hook);
	}
}

struct Hook<T: 'static> {
	arc: Weak<()>,
	accept: fn(Arc<()>, ReinitRef<T>),
	request_drop: fn(Arc<()>),
}

impl<T: 'static> Hook<T> {
	fn new<C>(arc: &Arc<C>, accept: fn(Arc<C>, ReinitRef<T>), request_drop: fn(Arc<C>)) -> Self {
		unsafe {
			Self {
				arc: transmute(Arc::downgrade(arc)),
				accept: transmute(accept),
				request_drop: transmute(request_drop),
			}
		}
	}

	fn accept(&self, t: ReinitRef<T>) -> bool {
		if let Some(arc) = self.arc.upgrade() {
			// SAFETY: call of function(Arc<C>) with Arc<()>
			(self.accept)(arc, t);
			true
		} else {
			false
		}
	}

	fn request_drop(&self) -> bool {
		if let Some(arc) = self.arc.upgrade() {
			// SAFETY: call of function(Arc<C>) with Arc<()>
			(self.request_drop)(arc);
			true
		} else {
			false
		}
	}
}

/// #[derive[Clone]) doesn't work as it requires T: Clone which it must not
impl<T> Clone for Reinit<T> {
	fn clone(&self) -> Self {
		Self { ptr: self.ptr.clone() }
	}
}

impl<T> Drop for Inner<T> {
	fn drop(&mut self) {
		// guarantees that self.instance has dropped already
		debug_assert_eq!(self.ref_cnt.load(Relaxed), 0, "self.t must have already dropped at this point");
	}
}


// Restart
pub struct Restart<T: 'static> {
	parent: WeakReinit<T>,
}

impl<T> Restart<T> {
	pub fn new(parent: &WeakReinit<T>) -> Self {
		Self { parent: parent.clone() }
	}

	pub fn restart(&self) {
		if let Some(parent) = self.parent.upgrade() {
			parent.restart();
		}
	}
}

/// #[derive[Clone]) doesn't work as it requires T: Clone which it must not
impl<T> Clone for Restart<T> {
	fn clone(&self) -> Self {
		Self { parent: self.parent.clone() }
	}
}

impl<T> Debug for Restart<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.write_str("Restart<")?;
		f.write_str(stringify!(T))?;
		f.write_str(">")
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

/// #[derive[Clone]) doesn't work as it requires T: Clone which it must not
impl<T> Clone for WeakReinit<T> {
	fn clone(&self) -> Self {
		Self { ptr: self.ptr.clone() }
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

	#[allow(clippy::mut_from_ref)]
	fn value(&self) -> &mut Option<ReinitRef<T>> {
		unsafe { &mut *self.value.get() }
	}

	#[inline]
	fn value_set(&self, t: ReinitRef<T>) {
		let cell = self.value();
		debug_assert!(matches!(cell, None));
		*cell = Some(t);
	}

	#[inline]
	fn value_clear(&self) {
		let cell = self.value();
		debug_assert!(matches!(cell, Some(..)));
		*cell = None;
	}

	#[inline]
	fn value_get(&self) -> &ReinitRef<T> {
		let cell = self.value();
		debug_assert!(matches!(cell, Some(..)));
		unsafe { cell.as_ref().unwrap_unchecked() }
	}
}


// Reinit0
struct Reinit0<T: 'static, F>
	where
		F: Fn(Restart<T>) -> T + 'static
{
	parent: WeakReinit<T>,
	constructor: F,
}

impl<T: 'static> Reinit<T>
{
	pub fn new0<F>(constructor: F) -> Reinit<T>
		where
			F: Fn(Restart<T>) -> T + 'static
	{
		Reinit::new(0, |weak| Arc::new(Reinit0 {
			parent: WeakReinit::new(weak),
			constructor,
		}), |_| {})
	}
}

impl<T: 'static, F> ReinitDetails<T> for Reinit0<T, F>
	where
		F: Fn(Restart<T>) -> T + 'static
{
	fn request_construction(&self, parent: &Reinit<T>) {
		parent.constructed((self.constructor)(Restart::new(&self.parent)))
	}
}


// Reinit1
struct Reinit1<T: 'static, F, A: 'static>
	where
		F: Fn(ReinitRef<A>, Restart<T>) -> T + 'static
{
	parent: WeakReinit<T>,
	constructor: F,
	a: Dependency<A>,
}

impl<T: 'static> Reinit<T>
{
	pub fn new1<F, A: 'static>(a: &Reinit<A>, constructor: F) -> Reinit<T>
		where
			F: Fn(ReinitRef<A>, Restart<T>) -> T + 'static
	{
		Reinit::new(1, |weak| {
			Arc::new(Reinit1 {
				parent: WeakReinit::new(weak),
				a: Dependency::new(a.clone()),
				constructor,
			})
		}, |arc| {
			arc.a.reinit.add_callback(arc, Reinit1::accept_a, Reinit1::request_drop_a);
		})
	}
}

impl<T: 'static, F, A: 'static> ReinitDetails<T> for Reinit1<T, F, A>
	where
		F: Fn(ReinitRef<A>, Restart<T>) -> T + 'static
{
	fn request_construction(&self, parent: &Reinit<T>) {
		parent.constructed((self.constructor)(self.a.value_get().clone(), Restart::new(&self.parent)));
	}
}

impl<T: 'static, F, A: 'static> Reinit1<T, F, A>
	where
		F: Fn(ReinitRef<A>, Restart<T>) -> T + 'static
{
	fn accept_a(self: Arc<Self>, t: ReinitRef<A>) {
		if let Some(parent) = self.parent.upgrade() {
			self.a.value_set(t);
			parent.construct_countdown();
		}
	}

	fn request_drop_a(self: Arc<Self>) {
		if let Some(parent) = self.parent.upgrade() {
			self.a.value_clear();
			parent.construct_countup();
		}
	}
}


// Reinit2
struct Reinit2<T: 'static, F, A: 'static, B: 'static>
	where
		F: Fn(ReinitRef<A>, ReinitRef<B>, Restart<T>) -> T + 'static
{
	parent: WeakReinit<T>,
	constructor: F,
	a: Dependency<A>,
	b: Dependency<B>,
}

impl<T: 'static> Reinit<T>
{
	pub fn new2<F, A: 'static, B: 'static>(a: &Reinit<A>, b: &Reinit<B>, constructor: F) -> Reinit<T>
		where
			F: Fn(ReinitRef<A>, ReinitRef<B>, Restart<T>) -> T + 'static
	{
		Reinit::new(2, |weak| {
			Arc::new(Reinit2 {
				parent: WeakReinit::new(weak),
				a: Dependency::new(a.clone()),
				b: Dependency::new(b.clone()),
				constructor,
			})
		}, |arc| {
			arc.a.reinit.add_callback(arc, Reinit2::accept_a, Reinit2::request_drop_a);
			arc.b.reinit.add_callback(arc, Reinit2::accept_b, Reinit2::request_drop_b);
		})
	}
}

impl<T: 'static, F, A: 'static, B: 'static> ReinitDetails<T> for Reinit2<T, F, A, B>
	where
		F: Fn(ReinitRef<A>, ReinitRef<B>, Restart<T>) -> T + 'static
{
	fn request_construction(&self, parent: &Reinit<T>) {
		parent.constructed((self.constructor)(self.a.value_get().clone(), self.b.value_get().clone(), Restart::new(&self.parent)));
	}
}

impl<T: 'static, F, A: 'static, B: 'static> Reinit2<T, F, A, B>
	where
		F: Fn(ReinitRef<A>, ReinitRef<B>, Restart<T>) -> T + 'static
{
	fn accept_a(self: Arc<Self>, t: ReinitRef<A>) {
		if let Some(parent) = self.parent.upgrade() {
			self.a.value_set(t);
			parent.construct_countdown();
		}
	}

	fn request_drop_a(self: Arc<Self>) {
		if let Some(parent) = self.parent.upgrade() {
			self.a.value_clear();
			parent.construct_countup();
		}
	}
	fn accept_b(self: Arc<Self>, t: ReinitRef<B>) {
		if let Some(parent) = self.parent.upgrade() {
			self.b.value_set(t);
			parent.construct_countdown();
		}
	}

	fn request_drop_b(self: Arc<Self>) {
		if let Some(parent) = self.parent.upgrade() {
			self.b.value_clear();
			parent.construct_countup();
		}
	}
}
