use std::ops::Deref;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

use parking_lot::Mutex;

use crate::reinit;
use crate::reinit::{Reinit, ReinitRef, Restart, State};
use crate::reinit_no_restart;

#[derive(Default, Eq, PartialEq, Debug, Copy, Clone)]
struct Calls {
	new: i32,
	drop: i32,
	callback: i32,
	callback_drop: i32,
}

impl Calls {
	const fn def() -> Self {
		Self {
			new: 0,
			drop: 0,
			callback: 0,
			callback_drop: 0,
		}
	}
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
struct Shared {
	counter: i32,
	a: Calls,
	b: Calls,
	c: Calls,
	d: Calls,
}

type SharedRef = Mutex<Shared>;

impl Shared {
	const fn new() -> SharedRef {
		Mutex::new(Self::def())
	}

	const fn def() -> Self {
		Self {
			counter: 1,
			a: Calls::def(),
			b: Calls::def(),
			c: Calls::def(),
			d: Calls::def(),
		}
	}

	fn reset(&mut self) {
		*self = Default::default()
	}

	fn register<F>(&mut self, call: F)
		where
			F: Fn(&mut Self) -> &mut i32
	{
		let counter = self.counter;
		self.counter += 1;
		let var = call(self);
		assert_eq!(*var, 0, "Call already occupied!");
		*var = counter;
	}
}

impl Default for Shared {
	fn default() -> Self {
		Self::def()
	}
}

struct AT<T: 'static> {
	shared: &'static SharedRef,
	t: T,
}

impl<T> AT<T> {
	fn new(shared: &'static SharedRef, t: T) -> Self {
		shared.lock().register(|s| &mut s.a.new);
		Self { shared, t }
	}

	fn register_callbacks(reinit: &'static Reinit<Self>, shared: &'static SharedRef) {
		reinit.ensure_initialized();
		reinit.add_callback(
			shared,
			|shared, _a| shared.deref().lock().register(|s| &mut s.a.callback),
			|shared| shared.deref().lock().register(|s| &mut s.a.callback_drop),
		)
	}
}

impl<T> Drop for AT<T> {
	fn drop(&mut self) {
		self.shared.lock().register(|s| &mut s.a.drop);
	}
}

struct BT<T: 'static, A: 'static> {
	a: ReinitRef<A>,
	shared: &'static SharedRef,
	t: T,
}

impl<T, A> BT<T, A> {
	fn new(a: ReinitRef<A>, shared: &'static SharedRef, t: T) -> Self {
		shared.lock().register(|s| &mut s.b.new);
		Self { a, shared, t }
	}

	fn register_callbacks(reinit: &'static Reinit<Self>, shared: &'static SharedRef) {
		reinit.ensure_initialized();
		reinit.add_callback(
			shared,
			|shared, _a| shared.deref().lock().register(|s| &mut s.b.callback),
			|shared| shared.deref().lock().register(|s| &mut s.b.callback_drop),
		)
	}
}

impl<T, A> Drop for BT<T, A> {
	fn drop(&mut self) {
		self.shared.lock().register(|s| &mut s.b.drop);
	}
}

struct CT<T: 'static, A: 'static> {
	a: ReinitRef<A>,
	shared: &'static SharedRef,
	t: T,
}

impl<T, A> CT<T, A> {
	fn new(a: ReinitRef<A>, shared: &'static SharedRef, t: T) -> Self {
		shared.lock().register(|s| &mut s.c.new);
		Self { a, shared, t }
	}

	fn register_callbacks(reinit: &'static Reinit<Self>, shared: &'static SharedRef) {
		reinit.ensure_initialized();
		reinit.add_callback(
			shared,
			|shared, _a| shared.deref().lock().register(|s| &mut s.c.callback),
			|shared| shared.deref().lock().register(|s| &mut s.c.callback_drop),
		)
	}
}

impl<T, A> Drop for CT<T, A> {
	fn drop(&mut self) {
		self.shared.lock().register(|s| &mut s.c.drop);
	}
}

struct DT<T: 'static, B: 'static, C: 'static> {
	b: ReinitRef<B>,
	c: ReinitRef<C>,
	shared: &'static SharedRef,
	t: T,
}

impl<T, B, C> DT<T, B, C> {
	fn new(b: ReinitRef<B>, c: ReinitRef<C>, shared: &'static SharedRef, t: T) -> Self {
		shared.lock().register(|s| &mut s.d.new);
		Self { b, c, shared, t }
	}

	fn register_callbacks(reinit: &'static Reinit<Self>, shared: &'static SharedRef) {
		reinit.ensure_initialized();
		reinit.add_callback(
			shared,
			|shared, _a| shared.deref().lock().register(|s| &mut s.d.callback),
			|shared| shared.deref().lock().register(|s| &mut s.d.callback_drop),
		)
	}
}

impl<T, B, C> Drop for DT<T, B, C> {
	fn drop(&mut self) {
		self.shared.lock().register(|s| &mut s.d.drop);
	}
}

#[test]
fn test_shared_reset() {
	let mut shared = Shared {
		b: Calls {
			callback_drop: 0,
			drop: 1,
			new: 2,
			callback: 3,
		},
		counter: 4,
		..Default::default()
	};

	shared.reset();
	assert_eq!(shared, Shared::default());
}

mod test_functions_panic {
	use super::*;

	reinit!(REINIT: i32 = {
		panic!("Should never construct")
	});

	#[test]
	#[should_panic]
	fn test_ref_panic() {
		assert!(matches!(REINIT.get_state(), State::Initialized));
		REINIT.test_ref();
	}

	#[test]
	#[should_panic]
	fn test_instance_panic() {
		assert!(matches!(REINIT.get_state(), State::Initialized));
		REINIT.test_instance();
	}
}


mod reinit0_basic {
	use super::*;

	static INITED: AtomicBool = AtomicBool::new(false);
	reinit!(REINIT: () = () => |_| {
		INITED.store(true, Relaxed);
	});

	#[test]
	fn reinit0_basic() {
		assert!(!INITED.load(Relaxed));

		let _need = REINIT.test_need();
		assert!(matches!(REINIT.get_state(), State::Initialized));
		assert_eq!(REINIT.countdown.load(Relaxed), 0);
		assert!(INITED.load(Relaxed));
		assert!(!REINIT.queued_restart.load(Relaxed));
		assert!(REINIT.ref_cnt.load(Relaxed) > 0);
	}
}

mod reinit0_reset_manual {
	use super::*;

	struct Shared {
		a: Option<&'static Reinit<i32>>,
		is_callback_init: bool,
		next_value: i32,
		received: Option<i32>,
		freed: bool,
		restart: Option<Restart<i32>>,
	}

	static RESTARTING: AtomicBool = AtomicBool::new(false);
	static SHARED: Mutex<Shared> = Mutex::new(Shared { a: None, is_callback_init: true, next_value: 42, received: None, freed: false, restart: None });
	reinit!(A: i32 = () => |restart| {
		let mut s = SHARED.lock();
		s.restart = Some(restart);
		s.next_value
	});

	#[test]
	fn reinit0_reset_manual() {
		SHARED.lock().a = Some(&A);
		let _need = A.test_need();
		assert!(matches!(A.get_state(), State::Initialized));

		// add callback
		A.add_callback(&SHARED, |shared, v| {
			let mut s = shared.lock();
			if RESTARTING.load(Relaxed) {
				assert!(matches!(s.a.as_ref().unwrap().get_state(), State::Constructing));
			} else {
				assert!(matches!(s.a.as_ref().unwrap().get_state(), State::Initialized));
			}
			assert_eq!(s.received, None);
			s.received = Some(**v);
		}, |shared| {
			let mut s = shared.lock();
			assert!(matches!(s.a.as_ref().unwrap().get_state(), State::Destructing));
			assert!(!s.freed);
			assert_eq!(s.received, None, "must not give value and then clear it");
			s.freed = true;
		});
		{
			let mut s = SHARED.lock();
			s.is_callback_init = false;
			assert_eq!(s.received, Some(42));
			assert!(!s.freed);
		}

		// restart
		let restart;
		{
			let mut s = SHARED.lock();
			s.next_value = 127;
			s.received = None;
			restart = *s.restart.as_ref().unwrap();
		}
		RESTARTING.store(true, Relaxed);
		restart.restart();
		RESTARTING.store(false, Relaxed);
		{
			let s = SHARED.lock();
			assert!(matches!(A.get_state(), State::Initialized));
			assert!(s.freed);
			assert_eq!(s.received, Some(127));
		}

		// drop
		{
			let mut s = SHARED.lock();
			s.freed = false;
			s.received = None;
		}
		drop(_need);
		{
			let s = SHARED.lock();
			assert!(matches!(A.get_state(), State::Uninitialized));
			assert!(s.freed);
			assert_eq!(s.received, None);
		}
	}
}

mod reinit0_restart {
	use super::*;

	type A = AT<()>;

	static SHARED: SharedRef = Shared::new();
	reinit!(A: A = A::new(&SHARED, ()));

	#[test]
	fn reinit0_restart() {
		A::register_callbacks(&A, &SHARED);

		// init
		let _need = A.test_need();
		assert_eq!(*SHARED.lock(), Shared {
			a: Calls {
				new: 1,
				callback: 2,
				..Default::default()
			},
			counter: 3,
			..Default::default()
		});
		SHARED.lock().reset();

		// restart A
		A.test_restart();
		assert_eq!(*SHARED.lock(), Shared {
			a: Calls {
				callback_drop: 1,
				drop: 2,
				new: 3,
				callback: 4,
			},
			counter: 5,
			..Default::default()
		});
		SHARED.lock().reset();

		// drop
		drop(_need);
		assert_eq!(*SHARED.lock(), Shared {
			a: Calls {
				callback_drop: 1,
				drop: 2,
				..Default::default()
			},
			counter: 3,
			..Default::default()
		});
		SHARED.lock().reset();
	}
}

mod reinit0_need {
	use super::*;

	static FREED: AtomicBool = AtomicBool::new(false);

	struct Bla {}

	impl Drop for Bla {
		fn drop(&mut self) {
			FREED.store(true, Relaxed);
		}
	}

	reinit!(REINIT: Bla = Bla {});

	#[test]
	fn reinit0_need() {
		assert!(matches!(REINIT.get_state(), State::Uninitialized));
		assert!(!FREED.load(Relaxed));

		// init
		let _need = REINIT.test_need();
		assert!(matches!(REINIT.get_state(), State::Initialized));
		assert!(!FREED.load(Relaxed));

		// drop
		drop(_need);
		assert!(matches!(REINIT.get_state(), State::Uninitialized));
		assert!(FREED.load(Relaxed), "T was not freed by Reinit!");
	}
}

mod reinit1_basic {
	use std::cell::Cell;

	use super::*;

	struct EvilCell(Cell<i32>);

	// SAFETY: only safe for this test
	unsafe impl Send for EvilCell {}

	// SAFETY: only safe for this test
	unsafe impl Sync for EvilCell {}

	type A = AT<EvilCell>;
	type B = BT<i32, A>;

	static SHARED: SharedRef = Shared::new();
	reinit!(A: A = A::new(&SHARED, EvilCell(Cell::new(0))));
	reinit!(B: B = (A: A) => |a, _| B::new(a.clone(), &SHARED, 2));

	#[test]
	fn reinit1_basic() {
		assert!(matches!(A.get_state(), State::Uninitialized));
		assert!(matches!(B.get_state(), State::Uninitialized));

		// init
		let _need = B.test_need();
		assert!(matches!(A.get_state(), State::Initialized));
		assert!(matches!(B.get_state(), State::Initialized));

		assert_eq!(A.test_instance() as *const A, B.test_instance().a.deref() as *const A);
		assert_eq!(A.test_instance().t.0.get(), 0);
		A.test_instance().t.0.set(42);
		assert_eq!(B.test_instance().a.t.0.get(), 42);
		assert_eq!(B.test_instance().t, 2);

		// drop
		drop(_need);
		assert!(matches!(A.get_state(), State::Uninitialized));
		assert!(matches!(B.get_state(), State::Uninitialized));
	}
}

mod reinit1_restart {
	use super::*;

	type A = AT<()>;
	type B = BT<(), A>;

	static SHARED: SharedRef = Shared::new();
	reinit!(A: A = A::new(&SHARED, ()));
	reinit!(B: B = (A: A) => |a, _| B::new(a.clone(), &SHARED, ()));

	#[test]
	fn reinit1_restart() {
		assert!(matches!(A.get_state(), State::Uninitialized));
		assert!(matches!(B.get_state(), State::Uninitialized));

		A::register_callbacks(&A, &SHARED);
		B::register_callbacks(&B, &SHARED);


		// init
		let _need = B.test_need();
		assert!(matches!(A.get_state(), State::Initialized));
		assert!(matches!(B.get_state(), State::Initialized));

		assert_eq!(*SHARED.lock(), Shared {
			a: Calls {
				new: 1,
				callback: 2,
				..Default::default()
			},
			b: Calls {
				new: 3,
				callback: 4,
				..Default::default()
			},
			counter: 5,
			..Default::default()
		});
		SHARED.lock().reset();

		// restart
		A.test_restart();
		assert_eq!(*SHARED.lock(), Shared {
			a: Calls {
				callback_drop: 1,
				drop: 4,
				new: 5,
				callback: 6,
			},
			b: Calls {
				callback_drop: 2,
				drop: 3,
				new: 7,
				callback: 8,
			},
			counter: 9,
			..Default::default()
		});
		SHARED.lock().reset();

		// drop
		drop(_need);
		assert!(matches!(A.get_state(), State::Uninitialized));
		assert!(matches!(B.get_state(), State::Uninitialized));
		assert_eq!(*SHARED.lock(), Shared {
			a: Calls {
				callback_drop: 3,
				drop: 4,
				..Default::default()
			},
			b: Calls {
				callback_drop: 1,
				drop: 2,
				..Default::default()
			},
			counter: 5,
			..Default::default()
		});
		SHARED.lock().reset();
	}
}

mod reinit2_diamond {
	use std::mem::swap;

	use super::*;

	type A = AT<()>;
	type B = BT<(), A>;
	type C = CT<(), A>;
	type D = DT<(), B, C>;

	mod bc {
		use super::*;

		static SHARED: SharedRef = Shared::new();
		reinit!(A: A = A::new(&SHARED, ()));
		reinit!(B: B = (A: A) => |a, _| B::new(a.clone(), &SHARED, ()));
		reinit!(C: C = (A: A) => |a, _| C::new(a.clone(), &SHARED, ()));
		reinit!(D: D = (B: B, C: C) => |b, c, _| D::new(b.clone(), c.clone(), &SHARED, ()));

		#[test]
		fn reinit2_diamond_b_then_c() {
			reinit2_diamond(&SHARED, &A, &B, &C, &D, true);
		}
	}

	mod cb {
		use super::*;

		static SHARED: SharedRef = Shared::new();
		reinit!(A: A = A::new(&SHARED, ()));
		reinit!(B: B = (A: A) => |a, _| B::new(a.clone(), &SHARED, ()));
		reinit!(C: C = (A: A) => |a, _| C::new(a.clone(), &SHARED, ()));
		reinit!(D: D = (B: B, C: C) => |b, c, _| D::new(b.clone(), c.clone(), &SHARED, ()));

		#[test]
		fn reinit2_diamond_c_then_b() {
			reinit2_diamond(&SHARED, &A, &B, &C, &D, false);
		}
	}

	fn reinit2_diamond(shared: &'static SharedRef, a: &'static Reinit<A>, b: &'static Reinit<B>, c: &'static Reinit<C>, d: &'static Reinit<D>, b_then_c: bool) {
		A::register_callbacks(a, shared);
		if b_then_c {
			B::register_callbacks(b, shared);
			C::register_callbacks(c, shared);
		} else {
			C::register_callbacks(c, shared);
			B::register_callbacks(b, shared);
		}
		D::register_callbacks(d, shared);

		// init
		let _need = d.test_need();
		{
			let expected = Shared {
				a: Calls {
					new: 1,
					callback: 2,
					..Default::default()
				},
				b: Calls {
					new: 3,
					callback: 4,
					..Default::default()
				},
				c: Calls {
					new: 5,
					callback: 6,
					..Default::default()
				},
				d: Calls {
					new: 7,
					callback: 8,
					..Default::default()
				},
				counter: 9,
			};
			// do not swap: need_inc() order is not influenced by callbacks, but by the generics "list"
			let mut shared = shared.lock();
			assert_eq!(*shared, expected);
			shared.reset();
		}

		// restart
		a.test_restart();
		{
			let mut expected = Shared {
				a: Calls {
					callback_drop: 1,
					drop: 8,
					new: 9,
					callback: 10,
				},
				b: Calls {
					callback_drop: 2,
					drop: 5,
					new: 11,
					callback: 12,
				},
				c: Calls {
					callback_drop: 6,
					drop: 7,
					new: 13,
					callback: 14,
				},
				d: Calls {
					callback_drop: 3,
					drop: 4,
					new: 15,
					callback: 16,
				},
				counter: 17,
			};
			if !b_then_c {
				swap(&mut expected.b, &mut expected.c);
			}
			let mut shared = shared.lock();
			assert_eq!(*shared, expected);
			shared.reset();
		}

		// drop
		drop(_need);
		{
			let expected = Shared {
				a: Calls {
					callback_drop: 7,
					drop: 8,
					..Default::default()
				},
				b: Calls {
					callback_drop: 3,
					drop: 4,
					..Default::default()
				},
				c: Calls {
					callback_drop: 5,
					drop: 6,
					..Default::default()
				},
				d: Calls {
					callback_drop: 1,
					drop: 2,
					..Default::default()
				},
				counter: 9,
			};
			// do not swap: need_inc() order is not influenced by callbacks, but by the generics "list"
			let mut shared = shared.lock();
			assert_eq!(*shared, expected);
			shared.reset();
		}
	}
}

mod reinit_restart_during_construction {
	use super::*;

	type A = AT<i32>;

	#[derive(Default)]
	struct State {
		restart_cnt: i32,
		shared_during_restart: Shared,
		shared_before_restart: Shared,
	}

	static STATE: Mutex<State> = Mutex::new(State {
		restart_cnt: 0,
		shared_during_restart: Shared::def(),
		shared_before_restart: Shared::def(),
	});
	static SHARED: SharedRef = Shared::new();

	reinit!(A: A = () => |restart| {
		let mut state = STATE.lock();
		let mut shared = SHARED.lock();
		shared.register(|s| &mut s.a.new);

		let restart_cnt = state.restart_cnt;
		state.restart_cnt += 1;
		match restart_cnt {
			1 => {
				state.shared_before_restart = *shared;
				shared.reset();
				restart.restart();
			}
			2 => {
				state.shared_during_restart = *shared;
				shared.reset();
			}
			_ => {}
		}
		AT { shared: &SHARED, t: restart_cnt }
	});

	#[test]
	fn reinit_restart_during_construction() {
		A::register_callbacks(&A, &SHARED);

		// init
		let _need = A.test_need();
		{
			let state = STATE.lock();
			let mut shared = SHARED.lock();
			assert_eq!(state.restart_cnt, 1);
			assert_eq!(A.test_instance().t, state.restart_cnt - 1);
			assert_eq!(*shared, Shared {
				a: Calls {
					new: 1,
					callback: 2,
					..Default::default()
				},
				counter: 3,
				..Default::default()
			});
			assert_eq!(state.shared_before_restart, Default::default());
			assert_eq!(state.shared_during_restart, Default::default());
			shared.reset();
		}

		// restart
		A.test_restart();
		{
			let state = STATE.lock();
			let shared = SHARED.lock();
			assert_eq!(state.restart_cnt, 3);
			assert_eq!(A.test_instance().t, state.restart_cnt - 1);
			assert_eq!(state.shared_before_restart, Shared {
				a: Calls {
					callback_drop: 1,
					drop: 2,
					new: 3,
					callback: 0,
				},
				counter: 4,
				..Default::default()
			});
			assert_eq!(state.shared_during_restart, Shared {
				a: Calls {
					callback_drop: 2,
					drop: 3,
					new: 4,
					callback: 1,
				},
				counter: 5,
				..Default::default()
			});
			assert_eq!(*shared, Shared {
				a: Calls {
					callback_drop: 0,
					drop: 0,
					new: 0,
					callback: 1,
				},
				counter: 2,
				..Default::default()
			});
		}
	}
}

mod reinit_restart_while_not_needed {
	use super::*;

	type A = AT<i32>;

	static ALLOW_CONSTRUCT: AtomicBool = AtomicBool::new(false);
	static SHARED: SharedRef = Shared::new();

	reinit!(A: A = () => |_| {
		if !ALLOW_CONSTRUCT.load(Relaxed) {
			panic!("Not allowed to construct!");
		}
		AT::new(&SHARED, 42)
	});

	#[test]
	fn reinit_restart_while_not_needed() {
		A::register_callbacks(&A, &SHARED);

		// restart without need should noop
		A.restart();
		assert!(matches!(A.get_state(), State::Uninitialized));
		assert_eq!(*SHARED.lock(), Shared::def());

		// needing later should not restart
		ALLOW_CONSTRUCT.store(true, Relaxed);
		let _need = A.test_need();
		assert!(matches!(A.get_state(), State::Initialized));
		let expected = Shared {
			a: Calls {
				new: 1,
				callback: 2,
				..Default::default()
			},
			counter: 3,
			..Default::default()
		};
		assert_eq!(*SHARED.lock(), expected);
	}
}

mod reinit_no_restart_basic {
	use super::*;

	static INITED: AtomicBool = AtomicBool::new(false);
	reinit_no_restart!(REINIT: () = {
		INITED.store(true, Relaxed);
	});

	#[test]
	fn reinit_no_restart_basic() {
		let _need = REINIT.test_need();

		assert!(matches!(REINIT.get_state(), State::Initialized));
		assert_eq!(REINIT.countdown.load(Relaxed), 0);
		assert!(INITED.load(Relaxed));
		assert!(!REINIT.queued_restart.load(Relaxed));
		assert!(REINIT.ref_cnt.load(Relaxed) > 0);
	}
}

mod reinit_no_restart_restarted {
	use super::*;

	reinit_no_restart!(REINIT: i32 = 42);

	#[test]
	#[should_panic(expected = "Constructed more than once!")]
	fn reinit_no_restart_restarted() {
		let _need = REINIT.test_need();
		assert!(matches!(REINIT.get_state(), State::Initialized));
		REINIT.test_restart();
	}
}
