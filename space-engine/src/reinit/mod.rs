use std::cell::{Cell, UnsafeCell};
use std::fmt::{Debug, Display, Formatter};
use std::hint::spin_loop;
use std::marker::PhantomData;
use std::mem::{MaybeUninit, replace, transmute};
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};

use parking_lot::{RawMutex, RawThreadId, ReentrantMutex};
use parking_lot::lock_api::ReentrantMutexGuard;

pub use target::*;
pub use variants::*;

pub trait Callback<T: 'static, D: ReinitDetails<T, D>> {
	fn accept(&self, t: ReinitRef<T, D>);

	fn request_drop(&self);
}

pub struct Reinit<T: 'static, D: ReinitDetails<T, D>>
{
	// members for figuring out if this works
	/// counter for how many times this is used by others, >0 means that this reinit should start, =0 that it should stop
	need_count: AtomicU32,
	/// The first time needCount is incremented, hooks must be registered. This bool is used to signal that registration has already happened.
	/// Sadly it does not seem to be possible to generate these hook lists during compile time, so it's done during first use.
	registered_hooks: AtomicBool,

	// members contributing to next state computation
	/// countdown to 0 for construction of dependent Reinits, may also be incremented to destruct instance
	countdown: AtomicU32,
	/// true if self should restart, restart happens when self would go into state Initialized
	queued_restart: AtomicBool,

	// state
	/// lock while constructing or destroying instance, may also lock hooks while holding this
	state_lock: ReentrantMutex<Cell<State>>,
	/// ref count for member instance
	ref_cnt: AtomicUsize,
	/// instance of T and holding an Arc reference to this to prevent freeing self
	instance: UnsafeCell<MaybeUninit<T>>,

	/// hooks of everyone wanting to get notified, unordered
	hooks: ReentrantMutex<UnsafeCell<Vec<Hook<T, D>>>>,

	details: D,
}

#[repr(u8)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum State {
	Uninitialized,
	Constructing,
	Initialized,
	Destructing,
}

pub trait ReinitDetails<T: 'static, D: ReinitDetails<T, D>>: 'static {
	fn init(&'static self, parent: &'static Reinit<T, D>);

	fn on_need_inc(&'static self, parent: &'static Reinit<T, D>);

	fn on_need_dec(&'static self, parent: &'static Reinit<T, D>);

	fn request_construction(&'static self, parent: &'static Reinit<T, D>);
}


// ReinitRef
#[repr(transparent)]
pub struct ReinitRef<T: 'static, D: ReinitDetails<T, D>> {
	inner: &'static Reinit<T, D>,
}

impl<T, D: ReinitDetails<T, D>> ReinitRef<T, D> {
	/// SAFETY: inner has to be in state Initialized
	#[inline]
	unsafe fn new(inner: &'static Reinit<T, D>) -> Self {
		inner.ref_inc();
		Self { inner }
	}

	#[inline]
	fn inner(&self) -> &'static Reinit<T, D> {
		self.inner
	}
}

impl<T, D: ReinitDetails<T, D>> Clone for ReinitRef<T, D> {
	#[inline]
	fn clone(&self) -> Self {
		unsafe {
			// SAFETY: inner has to be Initialized for self to exist
			ReinitRef::new(self.inner())
		}
	}
}

impl<T, D: ReinitDetails<T, D>> Drop for ReinitRef<T, D> {
	#[inline]
	fn drop(&mut self) {
		unsafe { self.inner().ref_dec() }
	}
}

impl<T, D: ReinitDetails<T, D>> Deref for ReinitRef<T, D> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		unsafe { self.inner().ref_get_instance() }
	}
}

impl<T: Debug, D: ReinitDetails<T, D>> Debug for ReinitRef<T, D> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.deref().fmt(f)
	}
}

impl<T: Display, D: ReinitDetails<T, D>> Display for ReinitRef<T, D> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.deref().fmt(f)
	}
}

impl<T, D: ReinitDetails<T, D>> Reinit<T, D> {
	#[inline]
	unsafe fn ref_inc(&self) {
		debug_assert!(self.ref_cnt.load(Relaxed) > 0);
		self.ref_cnt.fetch_add(1, Relaxed);
	}

	#[inline]
	unsafe fn ref_dec(&self) {
		debug_assert!(self.ref_cnt.load(Relaxed) > 0);
		if self.ref_cnt.fetch_sub(1, Relaxed) == 1 {
			// FIXME: do something with need here maybe?
			// (&*self.instance.get()).assume_init_ref().arc.slow_drop();
		}
	}

	#[inline]
	unsafe fn ref_get_instance(&self) -> &T {
		debug_assert!(self.ref_cnt.load(Relaxed) > 0);
		(*self.instance.get()).assume_init_ref()
	}
}


// Reinit impl
impl<T, D: ReinitDetails<T, D>> Reinit<T, D>
{
	#[inline]
	const fn new(initial_countdown: u32, details: D) -> Reinit<T, D>
	{
		Reinit {
			need_count: AtomicU32::new(0),
			registered_hooks: AtomicBool::new(false),
			countdown: AtomicU32::new(initial_countdown + 1),
			queued_restart: AtomicBool::new(false),
			state_lock: ReentrantMutex::new(Cell::new(State::Uninitialized)),
			ref_cnt: AtomicUsize::new(0),
			instance: UnsafeCell::new(MaybeUninit::uninit()),
			hooks: ReentrantMutex::new(UnsafeCell::new(vec![])),
			details,
		}
	}

	unsafe fn need_inc(&'static self) {
		loop {
			let need = self.need_count.load(Acquire);
			if need == 0 {
				// lock need_count for 0->1 transition
				if self.need_count.compare_exchange_weak(0, !0, Relaxed, Relaxed).is_ok() {
					// FIXME init details once for registering hooks
					if self.registered_hooks.compare_exchange(false, true, Relaxed, Relaxed).is_ok() {
						self.details.init(self);
					}

					self.details.on_need_inc(self);
					self.construct_dec();
					let unlock = self.need_count.compare_exchange(!0, 1, Release, Relaxed).is_ok();
					assert!(unlock, "need_count changed away from locked !0 value!");
					break;
				}
			} else if need == !0 {
				// locked from case above, busy wait
				spin_loop();
			} else {
				// need > 0: just increment
				if self.need_count.compare_exchange_weak(need, need + 1, Release, Relaxed).is_ok() {
					break;
				}
			}
		}
	}

	unsafe fn need_dec(&'static self) {
		loop {
			let need = self.need_count.load(Acquire);
			if need == 0 {
				panic!("need_count underflow!");
			} else if need == 1 {
				// lock need_count for 1->0 transition
				if self.need_count.compare_exchange_weak(1, !0, Relaxed, Relaxed).is_ok() {
					self.details.on_need_dec(self);
					self.construct_inc();
					let unlock = self.need_count.compare_exchange(!0, 0, Release, Relaxed).is_ok();
					assert!(unlock, "need_count changed away from locked !0 value!");
					break;
				}
			} else if need == !0 {
				// locked from case above, busy wait
				spin_loop();
			} else {
				// need > 1: just decrement
				if self.need_count.compare_exchange_weak(need, need - 1, Release, Relaxed).is_ok() {
					break;
				}
			}
		}
	}

	fn restart(&'static self) {
		// TODO restart called during construct/destruct must be handled correctly
		if self.queued_restart.compare_exchange(false, true, Relaxed, Relaxed).is_ok() {
			self.check_state();
		}
	}

	#[inline]
	fn construct_dec(&'static self) {
		if self.countdown.fetch_sub(1, Release) == 1 {
			self.check_state();
		}
	}

	#[inline]
	fn construct_inc(&'static self) {
		if self.countdown.fetch_add(1, Relaxed) == 0 {
			self.restart();
		}
	}

	fn check_state(&'static self) {
		// lock is held for the entire method
		let guard = self.state_lock.lock();
		if matches!(guard.get(), State::Constructing | State::Destructing) {
			// do nothing, wait for construction / destruction to finish
			return;
		}

		self.check_state_internal(&guard);
	}

	fn check_state_internal(&'static self, guard: &ReentrantMutexGuard<RawMutex, RawThreadId, Cell<State>>) {
		// figure out target state
		let is_init = guard.get() == State::Initialized;
		let mut should_be_init = self.countdown.load(Relaxed) == 0;
		// if self is initialized, self ALWAYS clear restart flag, even if I would have destructed anyways due to should_be_init = false.
		if is_init && self.queued_restart.compare_exchange(true, false, Relaxed, Relaxed).is_ok() {
			should_be_init = false;
		}
		// already in correct state -> do nothing
		if is_init == should_be_init {
			return;
		}

		// not in correct state
		if should_be_init {
			assert!(guard.get() == State::Uninitialized);
			guard.set(State::Constructing);

			self.details.request_construction(self);
		} else {
			assert!(guard.get() == State::Initialized);
			guard.set(State::Destructing);

			self.call_callbacks(|h| h.request_drop());

			// SAFETY: decrement initial ref count owned by ourselves
			// is required to be after call_callbacks() otherwise we may start constructing before calling all request_drop()s
			unsafe {
				self.ref_dec();
			}
		}
	}

	fn constructed(&'static self, t: T) {
		// lock is held for the entire method
		let guard = self.state_lock.lock();

		// change state
		assert!(guard.get() == State::Constructing);
		guard.set(State::Initialized);

		// initialize self.instance
		{
			assert_eq!(self.ref_cnt.load(Relaxed), 0);
			unsafe { &mut *self.instance.get() }.write(t);
			self.ref_cnt.store(1, Release);
		}

		// call hooks
		self.call_callbacks(|h| h.accept(unsafe {
			// SAFETY: we just made self be Initialized
			ReinitRef::new(self)
		}));

		// TODO do not call hooks if we should destruct immediately
		self.check_state_internal(&guard);
	}

	#[cold]
	#[inline(never)]
	fn slow_drop(&'static self) {
		// lock is held for the entire method
		let guard = self.state_lock.lock();

		// change state
		assert!(guard.get() == State::Destructing);
		guard.set(State::Uninitialized);

		// drop self.instance as noone is referencing it anymore
		unsafe {
			(*self.instance.get()).assume_init_drop()
		};

		self.check_state_internal(&guard);
	}

	fn call_callbacks<F>(&'static self, f: F)
		where
			F: Fn(&Hook<T, D>) -> bool
	{
		let lock = self.hooks.lock();
		let hooks = unsafe { &mut *lock.get() }; // BUG this is not safe!
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

	pub fn add_callback<C>(&'static self, arc: &Arc<C>, accept: fn(Arc<C>, ReinitRef<T, D>), request_drop: fn(Arc<C>))
	{
		let lock = self.hooks.lock();
		let hook = Hook::new(arc, accept, request_drop);
		if self.ref_cnt.load(Relaxed) > 0 {
			unsafe {
				// SAFETY: self being constructed is checked in the if above
				hook.accept(ReinitRef::new(self));
			}
		}
		let hooks = unsafe { &mut *lock.get() }; // BUG this is not safe!
		hooks.push(hook);
	}
}

struct Hook<T: 'static, D: ReinitDetails<T, D>> {
	arc: Weak<()>,
	accept: fn(Arc<()>, ReinitRef<T, D>),
	request_drop: fn(Arc<()>),
}

impl<T: 'static, D: ReinitDetails<T, D>> Hook<T, D> {
	fn new<C>(arc: &Arc<C>, accept: fn(Arc<C>, ReinitRef<T, D>), request_drop: fn(Arc<C>)) -> Self {
		unsafe {
			Self {
				arc: transmute(Arc::downgrade(arc)),
				accept: transmute(accept),
				request_drop: transmute(request_drop),
			}
		}
	}

	fn accept(&self, t: ReinitRef<T, D>) -> bool {
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

impl<T, D: ReinitDetails<T, D>> Drop for Reinit<T, D> {
	fn drop(&mut self) {
		// guarantees that self.instance has dropped already
		debug_assert_eq!(self.ref_cnt.load(Relaxed), 0, "self.t must have already dropped at this point");
	}
}


// Restart
pub struct Restart<T: 'static, D: ReinitDetails<T, D>> {
	parent: &'static Reinit<T, D>,
}

impl<T, D: ReinitDetails<T, D>> Restart<T, D> {
	pub fn new(parent: &'static Reinit<T, D>) -> Self {
		Self { parent }
	}

	pub fn restart(&self) {
		self.parent.restart();
	}
}

/// #[derive[Clone]) doesn't work as it requires T: Clone which it must not
impl<T, D: ReinitDetails<T, D>> Clone for Restart<T, D> {
	fn clone(&self) -> Self {
		Self { parent: self.parent }
	}
}

impl<T, D: ReinitDetails<T, D>> Copy for Restart<T, D> {}

impl<T, D: ReinitDetails<T, D>> Debug for Restart<T, D> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.write_str("Restart<")?;
		f.write_str(stringify!(T))?;
		f.write_str(">")
	}
}


// Dependency
struct Dependency<T: 'static, D: ReinitDetails<T, D>>
{
	reinit: Reinit<T, D>,
	value: UnsafeCell<Option<ReinitRef<T, D>>>,
}

impl<T, D: ReinitDetails<T, D>> Dependency<T, D> {
	fn new(reinit: Reinit<T, D>) -> Self {
		Self {
			reinit,
			value: UnsafeCell::new(None),
		}
	}

	#[allow(clippy::mut_from_ref)]
	fn value(&self) -> &mut Option<ReinitRef<T, D>> {
		unsafe { &mut *self.value.get() }
	}

	#[inline]
	fn value_set(&self, t: ReinitRef<T, D>) {
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
	fn value_get(&self) -> &ReinitRef<T, D> {
		let cell = self.value();
		debug_assert!(matches!(cell, Some(..)));
		unsafe { cell.as_ref().unwrap_unchecked() }
	}
}


// Reinit0
struct Reinit0<T: 'static, F>
	where
		F: Fn(Restart<T, Self>) -> T + 'static
{
	_phantom: PhantomData<T>,
	constructor: F,
}

impl<T: 'static, F> Reinit<T, Reinit0<T, F>>
	where
		F: Fn(Restart<T, Reinit0<T, F>>) -> T
{
	pub const fn new0(constructor: F) -> Reinit<T, Reinit0<T, F>>
	{
		Reinit::new(0, Reinit0 {
			_phantom: PhantomData {},
			constructor,
		})
	}
}

impl<T: 'static, F> ReinitDetails<T, Self> for Reinit0<T, F>
	where
		F: Fn(Restart<T, Self>) -> T
{
	fn init(&'static self, _: &'static Reinit<T, Self>) {}

	fn on_need_inc(&'static self, _: &'static Reinit<T, Self>) {}

	fn on_need_dec(&'static self, _: &'static Reinit<T, Self>) {}

	fn request_construction(&'static self, parent: &'static Reinit<T, Self>) {
		parent.constructed((self.constructor)(Restart::new(parent)))
	}
}


// ReinitNoRestart
struct ReinitNoRestart<T: 'static, F>
	where
		F: FnOnce() -> T + 'static
{
	constructor: UnsafeCell<Option<F>>,
}

impl<T: 'static, F> Reinit<T, ReinitNoRestart<T, F>>
	where
		F: FnOnce() -> T
{
	pub fn new_no_restart(constructor: F) -> Reinit<T, ReinitNoRestart<T, F>>
	{
		Reinit::new(0, ReinitNoRestart {
			constructor: UnsafeCell::new(Some(constructor))
		})
	}
}

impl<T: 'static, F> ReinitDetails<T, Self> for ReinitNoRestart<T, F>
	where
		F: FnOnce() -> T + 'static
{
	fn init(&'static self, _: &'static Reinit<T, Self>) {}

	fn on_need_inc(&'static self, _: &'static Reinit<T, Self>) {}

	fn on_need_dec(&'static self, _: &'static Reinit<T, Self>) {}

	fn request_construction(&'static self, parent: &'static Reinit<T, Self>) {
		let constructor = replace(unsafe { &mut *self.constructor.get() }, None);
		parent.constructed((constructor.expect("Constructed more than once!"))())
	}
}


// tests
#[cfg(test)]
impl<T, D: ReinitDetails<T, D>> Reinit<T, D> {
	#[inline]
	#[allow(clippy::mut_from_ref)]
	pub fn test_get_instance(&self) -> &mut T {
		assert!(self.ref_cnt.load(Relaxed) > 0);
		unsafe { &mut (&mut *self.instance.get()).assume_init_mut().instance }
	}

	#[inline]
	pub fn test_get_state(&self) -> State {
		self.state_lock.lock().get()
	}

	#[inline]
	pub fn test_restart(&self) {
		self.restart();
	}
}

mod target;
mod variants;

#[cfg(test)]
#[allow(dead_code)]
mod tests;
