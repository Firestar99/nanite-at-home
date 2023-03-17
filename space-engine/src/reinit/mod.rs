use std::cell::{Cell, UnsafeCell};
use std::fmt::{Debug, Display, Formatter};
use std::hint::spin_loop;
use std::mem::{forget, MaybeUninit, transmute};
use std::ops::Deref;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};

use parking_lot::{Mutex, MutexGuard};

pub use macros::*;
pub use no_restart::*;
pub use target::*;
pub use variants::*;

use crate::reinit::NeedIncType::{EnsureInitialized, NeedInc};

mod target;
mod variants;
mod macros;
mod no_restart;

#[cfg(test)]
#[allow(dead_code)]
mod tests;

pub struct Reinit<T: Send + Sync + 'static> {
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
	state_lock: Mutex<Cell<State>>,
	/// ref count for member instance
	ref_cnt: AtomicUsize,
	/// instance of T
	instance: UnsafeCell<MaybeUninit<T>>,

	/// hooks of everyone wanting to get notified, unordered
	hooks: Mutex<Vec<Hook<T, ()>>>,

	details: &'static dyn ReinitDetails<T>,
}

unsafe impl<T: Send + Sync + 'static> Send for Reinit<T> {}

unsafe impl<T: Send + Sync + 'static> Sync for Reinit<T> {}

#[repr(u8)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub enum State {
	Uninitialized,
	Constructing,
	Initialized,
	Destructing,
}

pub trait ReinitDetails<T: Send + Sync + 'static>: 'static {
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
pub struct ReinitRef<T: Send + Sync + 'static> {
	inner: &'static Reinit<T>,
}

impl<T: Send + Sync + 'static> ReinitRef<T> {
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

impl<T: Send + Sync + 'static> Clone for ReinitRef<T> {
	#[inline]
	fn clone(&self) -> Self {
		unsafe {
			// SAFETY: inner has to be Initialized for self to exist
			ReinitRef::new(self.inner())
		}
	}
}

impl<T: Send + Sync + 'static> Drop for ReinitRef<T> {
	#[inline]
	fn drop(&mut self) {
		unsafe {
			// SAFETY: inner has to be Initialized for self to exist
			self.inner().ref_dec()
		}
	}
}

impl<T: Send + Sync + 'static> Deref for ReinitRef<T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		unsafe {
			// SAFETY: inner has to be Initialized for self to exist
			self.inner().ref_get_instance()
		}
	}
}

impl<T: Debug + Send + Sync + 'static> Debug for ReinitRef<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.deref().fmt(f)
	}
}

impl<T: Display + Send + Sync + 'static> Display for ReinitRef<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.deref().fmt(f)
	}
}

impl<T: Send + Sync + 'static> Reinit<T> {
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
impl<T: Send + Sync + 'static> Reinit<T>
{
	#[inline]
	const fn new<D: ReinitDetails<T>>(initial_countdown: u32, details: &'static D) -> Reinit<T>
	{
		Reinit {
			need_count: AtomicU32::new(0),
			is_initialized: AtomicBool::new(false),
			countdown: AtomicU32::new(initial_countdown + 1),
			queued_restart: AtomicBool::new(false),
			state_lock: Mutex::new(Cell::new(State::Uninitialized)),
			ref_cnt: AtomicUsize::new(0),
			instance: UnsafeCell::new(MaybeUninit::uninit()),
			hooks: Mutex::new(vec![]),
			details,
		}
	}
}

#[derive(Clone, Copy)]
enum NeedIncType {
	NeedInc,
	EnsureInitialized,
}

impl<T: Send + Sync + 'static> Reinit<T>
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
		self.check_state_internal(self.state_lock.lock());
	}

	fn check_state_internal(&'static self, guard: MutexGuard<Cell<State>>) {
		if matches!(guard.get(), State::Constructing | State::Destructing) {
			// do nothing, wait for construction / destruction to finish
			return;
		}

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

			// total ordering continues to be ensured: as state is Destructing no other thread should mess with this object
			drop(guard);

			self.details.request_construction(self);
		} else {
			assert_eq!(guard.get(), State::Initialized);
			guard.set(State::Destructing);

			// total ordering continues to be ensured: as state is Destructing no other thread should mess with this object
			drop(guard);

			self.call_callbacks(|h| h.request_drop());

			// is required to be after call_callbacks() otherwise we may start constructing before calling all request_drop()s
			unsafe {
				// SAFETY: decrement initial ref count owned by ourselves
				self.ref_dec();
			}
		}
	}

	fn constructed(&'static self, t: T) {
		// initialize self.instance
		{
			{
				let instance = unsafe {
					// SAFETY: as ref_cnt == 0 no references must exist on instance, so we can grab &mut
					&mut *self.instance.get()
				};
				instance.write(t);
				// instance.assume_init_ref()
			}



			debug_assert_eq!(self.ref_cnt.load(Relaxed), 0);
			self.ref_cnt.store(1, Release);
		}
		// total ordering continues to be ensured: as state is still Destructing no other thread should mess with this object

		// call hooks
		// create and forget ReinitRef instance without ref counting, clone() it to actually inc ref count
		let reinit_ref = ReinitRef { inner: self };
		// TODO optimization: do not call hooks if we should destruct immediately
		self.call_callbacks(|h| h.accept(&reinit_ref));
		forget(reinit_ref);

		{
			let guard = self.state_lock.lock();
			// change state
			assert_eq!(guard.get(), State::Constructing);
			guard.set(State::Initialized);

			self.check_state_internal(guard);
		}
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

		self.check_state_internal(guard);
	}

	fn call_callbacks<F>(&'static self, f: F)
		where
			F: Fn(&Hook<T, ()>)
	{
		let lock = self.hooks.lock();
		lock.iter().for_each(f);
	}

	/// Removes a previously added callback using the callback's callee as the key.
	/// May be expensive as one needs to iterate over the entire vec of currently registered hooks. If need arises Reinit may have to switch to HashSets.
	pub fn remove_callback<C>(&'static self, callee: &'static C)
	{
		let mut lock = self.hooks.lock();
		for i in 0..lock.len() {
			if ptr::eq(lock.get(i).unwrap().callee, callee as *const _ as *const ()) {
				lock.swap_remove(i);
				return;
			}
		}
	}

	/// adds a new callback to this Reinit. It may be removed later with [remove_callback()] using the same `callee`
	/// * `callee`: a reference to some C that is passed to every function as the first argument, and used as a key to remove this callback again
	/// * `accept`: accept the new T value wrapped in an &ReinitRef<T>, which may be cloned to grab ownership of it
	/// * `request_drop`: if the ReinitRef<T> was taken ownership of in accept, it should be dropped to allow self to drop it's value for either restarting or stopping
	///
	/// One may NOT add or remove callbacks to this and only this Reinit within the accept or request_drop methods, as it will lead to a deadlock.
	///
	/// [remove_callback()]: Self::remove_callback
	pub fn add_callback<C>(&'static self, callee: &'static C, accept: fn(&'static C, &ReinitRef<T>), request_drop: fn(&'static C))
	{
		// needs to lock before calling accept on hook to ensure proper ordering
		let mut lock = self.hooks.lock();
		let hook = Hook::new(callee, accept, request_drop);
		if self.ref_cnt.load(Relaxed) > 0 {
			// create and forget ReinitRef instance without ref counting, clone() it to actually inc ref count
			let reinit_ref = ReinitRef { inner: self };
			hook.accept(&reinit_ref);
			forget(reinit_ref);
		}
		lock.push(hook.ungenerify());
	}
}

struct Hook<T: Send + Sync + 'static, C: 'static> {
	callee: &'static C,
	accept: fn(&'static C, &ReinitRef<T>),
	request_drop: fn(&'static C),
}

impl<T: Send + Sync + 'static, C: 'static> Hook<T, C> {
	fn new(callee: &'static C, accept: fn(&'static C, &ReinitRef<T>), request_drop: fn(&'static C)) -> Self {
		Self { callee, accept, request_drop }
	}

	fn ungenerify(self) -> Hook<T, ()> {
		// SAFETY: transmuting raw pointers to raw pointers, just the type they point to actually changes
		unsafe { transmute(self) }
	}

	fn accept(&self, t: &ReinitRef<T>) {
		// SAFETY: call of function(Arc<C>) with Arc<()>
		(self.accept)(self.callee, t);
	}

	fn request_drop(&self) {
		(self.request_drop)(self.callee);
	}
}

impl<T: Send + Sync + 'static> Drop for Reinit<T> {
	fn drop(&mut self) {
		// guarantees that self.instance has dropped already
		debug_assert_eq!(self.ref_cnt.load(Relaxed), 0, "self.t must have already dropped at this point");
	}
}


// Restart allows one to restart referenced Reinit
pub struct Restart<T: Send + Sync + 'static>(&'static Reinit<T>);

impl<T: Send + Sync + 'static> Restart<T> {
	pub fn restart(&self) {
		self.0.restart();
	}
}

/// #[derive[Clone]) doesn't work as it requires T: Clone which it must not
impl<T: Send + Sync + 'static> Clone for Restart<T> {
	fn clone(&self) -> Self {
		Self(self.0)
	}
}

impl<T: Send + Sync + 'static> Copy for Restart<T> {}

impl<T: Send + Sync + 'static> Debug for Restart<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.write_str("Restart<")?;
		f.write_str(stringify!(T))?;
		f.write_str(">")
	}
}


// Constructed allows access once to the constructed() method
pub struct Constructed<T: Send + Sync + 'static> (&'static Reinit<T>);

impl<T: Send + Sync + 'static> Constructed<T> {
	pub fn constructed(self, t: T) {
		self.0.constructed(t);
	}
}


// Dependency for variants to use
struct Dependency<T: Send + Sync + 'static>
{
	reinit: &'static Reinit<T>,
	value: UnsafeCell<Option<ReinitRef<T>>>,
}

impl<T: Send + Sync + 'static> Dependency<T> {
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
	fn value_ref(&self) -> &ReinitRef<T> {
		let cell = self.value();
		debug_assert!(matches!(cell, Some(..)));
		unsafe { cell.as_ref().unwrap_unchecked() }
	}
}


// Reinit0
pub struct Reinit0<T: Send + Sync + 'static>
{
	constructor: fn(Restart<T>, Constructed<T>),
}

impl<T: Send + Sync + 'static> Reinit0<T> {
	pub const fn new(constructor: fn(Restart<T>, Constructed<T>)) -> Self
	{
		Self { constructor }
	}

	pub const fn create_reinit(&'static self) -> Reinit<T> {
		Reinit::new(0, self)
	}
}

impl<T: Send + Sync + 'static> ReinitDetails<T> for Reinit0<T>
{
	fn init(&'static self, _: &'static Reinit<T>) {}

	unsafe fn on_need_inc(&'static self, _: &'static Reinit<T>) {}

	unsafe fn on_need_dec(&'static self, _: &'static Reinit<T>) {}

	fn request_construction(&'static self, parent: &'static Reinit<T>) {
		(self.constructor)(Restart::<T>(parent), Constructed(parent))
	}
}


// asserts and tests
impl<T: Send + Sync + 'static> Reinit<T> {
	#[inline]
	pub fn get_state(&'static self) -> State {
		self.state_lock.lock().get()
	}

	#[inline]
	pub fn assert_state(&'static self, state: State) {
		assert_eq!(self.get_state(), state);
	}
}

#[cfg(test)]
mod test_helper {
	use std::thread::sleep;
	use std::time::{Duration, Instant};

	use super::*;

	impl<T: Send + Sync + 'static> Reinit<T> {
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
		pub fn test_restart(&'static self) {
			self.test_restart_timeout(Duration::from_secs(1));
		}

		#[inline]
		pub fn test_restart_timeout(&'static self, timeout: Duration) {
			self.restart();
			// restart flag is consumed when construction initiates.
			// However when the clearing of this flag is visible, the state update to Constructing may not yet be!
			// But that's ok it will be at least in state Uninitialized which is all we need.
			// Also we may double the timeout but that's fine tests aren't time sensitive.
			busy_wait_loop(timeout, Some(|| String::from("Restart flag to be consumed")), || !self.queued_restart.load(Relaxed));
			busy_wait_loop(timeout, Some(|| String::from("Reinit Initialized after restart")), || self.get_state() == State::Initialized);
		}

		pub fn busy_wait_until_state(&'static self, state: State, timeout: Duration) {
			busy_wait_loop(timeout, Some(|| format!("Reinit in state {:?}", state)), || self.get_state() == state);
		}
	}

	pub fn busy_wait_loop<F, R>(timeout: Duration, timeout_reason: Option<R>, f: F)
		where
			F: Fn() -> bool,
			R: Fn() -> String,
	{
		let start = Instant::now();
		loop {
			if f() {
				return;
			}
			if start.elapsed() > timeout {
				let reason = timeout_reason.map(|r| (" while waiting for ", r())).unwrap_or_else(|| ("", String::new()));
				panic!("timeout after {:?}{}{}!", timeout, reason.0, reason.1);
			}
			// sleep(1ms) instead of spin_loop() as we don't know how long it may take
			// for a Reinit to initialize and need to back off from lock()-ing Mutexes
			sleep(Duration::from_millis(1));
		}
	}
}
