use std::cell::{Cell, UnsafeCell};
use std::fmt::{Debug, Display, Formatter};
use std::hint::spin_loop;
use std::mem::{MaybeUninit, replace, transmute};
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};

use parking_lot::{RawMutex, RawThreadId, ReentrantMutex};
use parking_lot::lock_api::ReentrantMutexGuard;

pub use target::*;
pub use variants::*;

use crate::reinit::NeedIncType::{EnsureInitialized, NeedInc};

mod target;
mod variants;

#[cfg(test)]
#[allow(dead_code)]
mod tests;

pub trait Callback<T: 'static> {
	fn accept(&self, t: ReinitRef<T>);

	fn request_drop(&self);
}

pub struct Reinit<T: 'static> {
	// members for figuring out if this works
	/// counter for how many times this is used by others, >0 means that this reinit should start, =0 that it should stop
	need_count: AtomicU32,
	/// The first time needCount is incremented, hooks must be registered. This bool is used to signal that registration has already happened and has been completed.
	/// Sadly it does not seem to be possible to generate these hook lists during compile time, so it's done during first use.
	is_initialized: AtomicBool,

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
	/// instance of T
	instance: UnsafeCell<MaybeUninit<T>>,

	/// hooks of everyone wanting to get notified, unordered
	hooks: ReentrantMutex<UnsafeCell<Vec<Hook<T, ()>>>>,

	details: &'static dyn ReinitDetails<T>,
}

unsafe impl<T> Send for Reinit<T> {}

unsafe impl<T> Sync for Reinit<T> {}

#[repr(u8)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub enum State {
	Uninitialized,
	Constructing,
	Initialized,
	Destructing,
}

pub trait ReinitDetails<T: 'static>: 'static {
	/// register callbacks on Dependencies
	fn init(&'static self, parent: &'static Reinit<T>);

	/// # Safety
	/// allow need_inc() calls on Dependencies
	unsafe fn on_need_inc(&'static self, parent: &'static Reinit<T>);

	/// # Safety
	/// allow need_dec() calls on Dependencies
	unsafe fn on_need_dec(&'static self, parent: &'static Reinit<T>);

	/// actually construct T
	fn request_construction(&'static self, parent: &'static Reinit<T>);
}


// ReinitRef
#[repr(transparent)]
pub struct ReinitRef<T: 'static> {
	inner: &'static Reinit<T>,
}

impl<T> ReinitRef<T> {
	/// SAFETY: inner has to be in state Initialized
	#[inline]
	unsafe fn new(inner: &'static Reinit<T>) -> Self {
		inner.ref_inc();
		Self { inner }
	}

	#[inline]
	fn inner(&self) -> &'static Reinit<T> {
		self.inner
	}
}

impl<T> Clone for ReinitRef<T> {
	#[inline]
	fn clone(&self) -> Self {
		unsafe {
			// SAFETY: inner has to be Initialized for self to exist
			ReinitRef::new(self.inner())
		}
	}
}

impl<T> Drop for ReinitRef<T> {
	#[inline]
	fn drop(&mut self) {
		unsafe {
			// SAFETY: inner has to be Initialized for self to exist
			self.inner().ref_dec()
		}
	}
}

impl<T> Deref for ReinitRef<T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		unsafe {
			// SAFETY: inner has to be Initialized for self to exist
			self.inner().ref_get_instance()
		}
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

impl<T> Reinit<T> {
	#[inline]
	unsafe fn ref_inc(&'static self) {
		debug_assert!(self.ref_cnt.load(Relaxed) > 0);
		self.ref_cnt.fetch_add(1, Relaxed);
	}

	#[inline]
	unsafe fn ref_dec(&'static self) {
		debug_assert!(self.ref_cnt.load(Relaxed) > 0);
		if self.ref_cnt.fetch_sub(1, Relaxed) == 1 {
			self.slow_drop();
		}
	}

	#[inline]
	unsafe fn ref_get_instance(&'static self) -> &T {
		debug_assert!(self.ref_cnt.load(Relaxed) > 0);
		(*self.instance.get()).assume_init_ref()
	}
}


// Reinit impl
impl<T> Reinit<T>
{
	#[inline]
	const fn new<D: ReinitDetails<T>>(initial_countdown: u32, details: &'static D) -> Reinit<T>
	{
		Reinit {
			need_count: AtomicU32::new(0),
			is_initialized: AtomicBool::new(false),
			countdown: AtomicU32::new(initial_countdown + 1),
			queued_restart: AtomicBool::new(false),
			state_lock: ReentrantMutex::new(Cell::new(State::Uninitialized)),
			ref_cnt: AtomicUsize::new(0),
			instance: UnsafeCell::new(MaybeUninit::uninit()),
			hooks: ReentrantMutex::new(UnsafeCell::new(vec![])),
			details,
		}
	}
}

#[derive(Clone, Copy)]
enum NeedIncType {
	NeedInc,
	EnsureInitialized,
}

impl<T> Reinit<T>
{
	unsafe fn need_inc(&'static self) {
		self.need_inc_internal(NeedInc)
	}

	pub fn ensure_initialized(&'static self) {
		unsafe { self.need_inc_internal(EnsureInitialized) }
	}

	#[inline]
	unsafe fn need_inc_internal(&'static self, inc_type: NeedIncType) {
		loop {
			match inc_type {
				NeedInc => {}
				EnsureInitialized => {
					if self.is_initialized.load(Relaxed) {
						// hooks already registered: return
						break;
					}
				}
			}

			let need = self.need_count.load(Acquire);
			if need == 0 {
				// lock need_count for 0->1 transition (or hooks registration)
				if self.need_count.compare_exchange_weak(0, !0, Relaxed, Relaxed).is_ok() {
					// register hooks: as this section is mutexed via need_count lock, we can update registered_hooks non-atomically
					// This allows us to only set registered_hooks to true once registering has finished, not just when we started
					if !self.is_initialized.load(Relaxed) {
						self.details.init(self);
						let hooks_done = self.is_initialized.compare_exchange(false, true, Relaxed, Relaxed).is_ok();
						assert!(hooks_done, "hooks were registered twice!")
					}

					let unlock_state = match inc_type {
						NeedInc => {
							self.details.on_need_inc(self);
							self.construct_dec();
							1
						}
						EnsureInitialized => 0
					};
					let unlock = self.need_count.compare_exchange(!0, unlock_state, Release, Relaxed).is_ok();
					assert!(unlock, "need_count changed away from locked !0 value!");
					break;
				}
			} else if need == !0 {
				// locked from case above, busy wait
				spin_loop();
			} else {
				match inc_type {
					NeedInc => {
						// need > 0: just increment
						if self.need_count.compare_exchange_weak(need, need + 1, Release, Relaxed).is_ok() {
							break;
						}
					}
					EnsureInitialized => {
						// need > 0: race condition between top registered_hooks check and need_count lock
						// by now hooks must have been registered by a different thread
						assert!(self.is_initialized.load(Relaxed));
						break;
					}
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
					self.construct_inc();
					self.details.on_need_dec(self);
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
		// only handle the first call to restart()
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
		// force should_be_init to false if we should restart, restart flag is cleared on construction start
		if is_init && self.queued_restart.load(Relaxed) {
			should_be_init = false;
		}
		// already in correct state -> do nothing
		if is_init == should_be_init {
			return;
		}

		// not in correct state
		if should_be_init {
			assert_eq!(guard.get(), State::Uninitialized);
			guard.set(State::Constructing);
			self.queued_restart.store(false, Relaxed);

			self.details.request_construction(self);
		} else {
			assert_eq!(guard.get(), State::Initialized);
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
		assert_eq!(guard.get(), State::Constructing);
		guard.set(State::Initialized);

		// initialize self.instance
		{
			assert_eq!(self.ref_cnt.load(Relaxed), 0);
			unsafe {
				// SAFETY: as ref_cnt == 0 no references must exist on instance
				&mut *self.instance.get()
			}.write(t);
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
		assert_eq!(guard.get(), State::Destructing);
		guard.set(State::Uninitialized);

		// drop self.instance as noone is referencing it anymore
		unsafe {
			(*self.instance.get()).assume_init_drop()
		};

		self.check_state_internal(&guard);
	}

	fn call_callbacks<F>(&'static self, f: F)
		where
			F: Fn(&Hook<T, ()>)
	{
		let lock = self.hooks.lock();
		let hooks = unsafe { &*lock.get() };
		hooks.iter().for_each(f);
	}

	pub fn add_callback<C>(&'static self, callee: &'static C, accept: fn(&'static C, ReinitRef<T>), request_drop: fn(&'static C))
	{
		let lock = self.hooks.lock();
		let hook = Hook::new(callee, accept, request_drop);
		if self.ref_cnt.load(Relaxed) > 0 {
			unsafe {
				// SAFETY: self being constructed is checked in the if above
				hook.accept(ReinitRef::new(self));
			}
		}
		// BUG this is not safe cause ReentrantMutex
		let hooks = unsafe { &mut *lock.get() };
		hooks.push(hook.ungenerify());
	}
}

struct Hook<T: 'static, C: 'static> {
	callee: &'static C,
	accept: fn(&'static C, ReinitRef<T>),
	request_drop: fn(&'static C),
}

impl<T: 'static, C: 'static> Hook<T, C> {
	fn new(callee: &'static C, accept: fn(&'static C, ReinitRef<T>), request_drop: fn(&'static C)) -> Self {
		Self { callee, accept, request_drop }
	}

	fn ungenerify(self) -> Hook<T, ()> {
		// SAFETY: transmuting raw pointers to raw pointers, just the type they point to actually changes
		unsafe { transmute(self) }
	}

	fn accept(&self, t: ReinitRef<T>) {
		// SAFETY: call of function(Arc<C>) with Arc<()>
		(self.accept)(self.callee, t);
	}

	fn request_drop(&self) {
		(self.request_drop)(self.callee);
	}
}

impl<T> Drop for Reinit<T> {
	fn drop(&mut self) {
		// guarantees that self.instance has dropped already
		debug_assert_eq!(self.ref_cnt.load(Relaxed), 0, "self.t must have already dropped at this point");
	}
}


// Restart
pub struct Restart<T: 'static> {
	parent: &'static Reinit<T>,
}

impl<T> Restart<T> {
	pub fn new(parent: &'static Reinit<T>) -> Self {
		Self { parent }
	}

	pub fn restart(&self) {
		self.parent.restart();
	}
}

/// #[derive[Clone]) doesn't work as it requires T: Clone which it must not
impl<T> Clone for Restart<T> {
	fn clone(&self) -> Self {
		Self { parent: self.parent }
	}
}

impl<T> Copy for Restart<T> {}

impl<T> Debug for Restart<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.write_str("Restart<")?;
		f.write_str(stringify!(T))?;
		f.write_str(">")
	}
}


// Dependency
struct Dependency<T: 'static>
{
	reinit: &'static Reinit<T>,
	value: UnsafeCell<Option<ReinitRef<T>>>,
}

impl<T> Dependency<T> {
	const fn new(reinit: &'static Reinit<T>) -> Self {
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
pub struct Reinit0<T: 'static>
{
	constructor: fn(Restart<T>) -> T,
}

impl<T: 'static> Reinit0<T> {
	pub const fn new(constructor: fn(Restart<T>) -> T) -> Self
	{
		Self { constructor }
	}

	pub const fn create_reinit(&'static self) -> Reinit<T> {
		Reinit::new(0, self)
	}
}

impl<T: 'static> ReinitDetails<T> for Reinit0<T>
{
	fn init(&'static self, _: &'static Reinit<T>) {}

	unsafe fn on_need_inc(&'static self, _: &'static Reinit<T>) {}

	unsafe fn on_need_dec(&'static self, _: &'static Reinit<T>) {}

	fn request_construction(&'static self, parent: &'static Reinit<T>) {
		parent.constructed((self.constructor)(Restart::new(parent)))
	}
}


// ReinitNoRestart
pub struct ReinitNoRestart<T: 'static>
{
	constructor: UnsafeCell<Option<fn() -> T>>,
}

// member constructor is not Sync
unsafe impl<T: 'static> Sync for ReinitNoRestart<T> {}

impl<T: 'static> ReinitNoRestart<T> {
	pub const fn new(constructor: fn() -> T) -> Self
	{
		Self {
			constructor: UnsafeCell::new(Some(constructor))
		}
	}

	pub const fn create_reinit(&'static self) -> Reinit<T> {
		Reinit::new(0, self)
	}
}

#[macro_export]
macro_rules! reinit_no_restart {
	($name:ident: $t:ty = $f:expr) => (paste::paste!{
		static [<$name _DETAILS>]: $crate::reinit::ReinitNoRestart<$t> = $crate::reinit::ReinitNoRestart::new(|| $f);
		static $name: $crate::reinit::Reinit<$t> = [<$name _DETAILS>].create_reinit();
	});
}

impl<T: 'static> ReinitDetails<T> for ReinitNoRestart<T>
{
	fn init(&'static self, _: &'static Reinit<T>) {}

	unsafe fn on_need_inc(&'static self, _: &'static Reinit<T>) {}

	unsafe fn on_need_dec(&'static self, _: &'static Reinit<T>) {}

	fn request_construction(&'static self, parent: &'static Reinit<T>) {
		// this may not be atomic, but that's ok as Reinit will act as a Mutex for this method
		let constructor = replace(unsafe { &mut *self.constructor.get() }, None);
		parent.constructed((constructor.expect("Constructed more than once!"))())
	}
}


// tests
#[cfg(test)]
impl<T> Reinit<T> {
	#[inline]
	pub fn test_instance(&self) -> &T {
		assert!(self.ref_cnt.load(Relaxed) > 0);
		unsafe {
			// SAFETY: asserted self is initialized above, BUT does not account for multithreading
			(*self.instance.get()).assume_init_ref()
		}
	}

	#[inline]
	pub fn test_ref(&'static self) -> ReinitRef<T> {
		assert!(self.ref_cnt.load(Relaxed) > 0);
		unsafe {
			// SAFETY: asserted self is initialized above, BUT does not account for multithreading
			ReinitRef::new(self)
		}
	}

	#[inline]
	pub fn test_get_state(&'static self) -> State {
		self.state_lock.lock().get()
	}

	#[inline]
	pub fn test_restart(&'static self) {
		self.restart();
	}
}
