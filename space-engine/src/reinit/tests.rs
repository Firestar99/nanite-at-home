use std::cell::{Cell, RefCell};
use std::mem::swap;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::Relaxed;

use crate::reinit::{Reinit, ReinitRef, Restart, State};

#[derive(Default, Eq, PartialEq, Debug, Copy, Clone)]
struct Calls {
	new: i32,
	drop: i32,
	callback: i32,
	callback_drop: i32,
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
struct Shared {
	counter: i32,
	a: Calls,
	b: Calls,
	c: Calls,
	d: Calls,
}

type SharedRef = Arc<RefCell<Shared>>;

impl Shared {
	fn new() -> SharedRef {
		Arc::new(RefCell::new(Shared { ..Default::default() }))
	}

	fn reset(&mut self) {
		*self = Shared { ..Default::default() }
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
		Self {
			counter: 1,
			a: Default::default(),
			b: Default::default(),
			c: Default::default(),
			d: Default::default(),
		}
	}
}

struct AT<T: 'static> {
	shared: SharedRef,
	t: T,
}

impl<T: Clone> AT<T> {
	fn new(shared: &SharedRef, t: T) -> Reinit<Self> {
		let shared_a = shared.clone();
		let a = Reinit::new0(move |_| {
			shared_a.deref().borrow_mut().register(|s| &mut s.a.new);
			AT { shared: shared_a.clone(), t: t.clone() }
		});
		a.add_callback(
			shared,
			|shared, _a| shared.deref().borrow_mut().register(|s| &mut s.a.callback),
			|shared| shared.deref().borrow_mut().register(|s| &mut s.a.callback_drop),
		);
		a
	}
}

impl<T> Drop for AT<T> {
	fn drop(&mut self) {
		self.shared.deref().borrow_mut().register(|s| &mut s.a.drop);
	}
}

struct BT<T: 'static, A: 'static> {
	shared: SharedRef,
	a: ReinitRef<A>,
	t: T,
}

impl<T: Clone, A> BT<T, A> {
	fn new(shared: &SharedRef, a: &Reinit<A>, t: T) -> Reinit<Self> {
		let shared_a = shared.clone();
		let b = Reinit::new1(a, move |a, _| {
			shared_a.deref().borrow_mut().register(|s| &mut s.b.new);
			BT { a, shared: shared_a.clone(), t: t.clone() }
		});
		b.add_callback(
			shared,
			|shared, _b| shared.deref().borrow_mut().register(|s| &mut s.b.callback),
			|shared| shared.deref().borrow_mut().register(|s| &mut s.b.callback_drop),
		);
		b
	}
}

impl<T, A> Drop for BT<T, A> {
	fn drop(&mut self) {
		self.shared.deref().borrow_mut().register(|s| &mut s.b.drop);
	}
}

struct CT<T: 'static, A: 'static> {
	shared: SharedRef,
	a: ReinitRef<A>,
	t: T,
}

impl<T: Clone, A> CT<T, A> {
	fn new(shared: &SharedRef, a: &Reinit<A>, t: T) -> Reinit<Self> {
		let shared_a = shared.clone();
		let c = Reinit::new1(a, move |a, _| {
			shared_a.deref().borrow_mut().register(|s| &mut s.c.new);
			CT { a, shared: shared_a.clone(), t: t.clone() }
		});
		c.add_callback(
			shared,
			|shared, _c| shared.deref().borrow_mut().register(|s| &mut s.c.callback),
			|shared| shared.deref().borrow_mut().register(|s| &mut s.c.callback_drop),
		);
		c
	}
}

impl<T, A> Drop for CT<T, A> {
	fn drop(&mut self) {
		self.shared.deref().borrow_mut().register(|s| &mut s.c.drop);
	}
}

struct DT<T: 'static, B: 'static, C: 'static> {
	shared: SharedRef,
	b: ReinitRef<B>,
	c: ReinitRef<C>,
	t: T,
}

impl<T: Clone, B, C> DT<T, B, C> {
	fn new(shared: &SharedRef, b: &Reinit<B>, c: &Reinit<C>, t: T) -> Reinit<Self> {
		let shared_a = shared.clone();
		let d = Reinit::new2(b, c, move |b, c, _| {
			shared_a.deref().borrow_mut().register(|s| &mut s.d.new);
			DT { b, c, shared: shared_a.clone(), t: t.clone() }
		});
		d.add_callback(
			shared,
			|shared, _d| shared.deref().borrow_mut().register(|s| &mut s.d.callback),
			|shared| shared.deref().borrow_mut().register(|s| &mut s.d.callback_drop),
		);
		d
	}
}

impl<T, B, C> Drop for DT<T, B, C> {
	fn drop(&mut self) {
		self.shared.deref().borrow_mut().register(|s| &mut s.d.drop);
	}
}

#[test]
fn test_shared_reset() {
	let shared = Shared::new();
	*shared.deref().borrow_mut() = Shared {
		b: Calls {
			callback_drop: 0,
			drop: 1,
			new: 2,
			callback: 3,
		},
		counter: 4,
		..Default::default()
	};

	shared.deref().borrow_mut().reset();
	assert_eq!(*shared.deref().borrow_mut(), Shared {
		..Default::default()
	});
}

#[test]
fn reinit0_basic() {
	let inited = Arc::new(Cell::new(false));
	let inited2 = inited.clone();
	let reinit = Reinit::new0(move |_| {
		inited2.set(true);
	});
	assert!(matches!(reinit.test_get_state(), State::Initialized));
	assert_eq!(reinit.ptr.countdown.load(Relaxed), 0);
	assert!(inited.get());
	assert!(!reinit.ptr.queued_restart.load(Relaxed));
	assert!(reinit.ptr.ref_cnt.load(Relaxed) > 0);
}

#[test]
fn reinit0_reset_manual() {
	struct Shared {
		a: Option<Reinit<i32>>,
		is_callback_init: bool,
		next_value: i32,
		received: Option<i32>,
		freed: bool,
		restart: Option<Restart<i32>>,
	}
	let shared = Arc::new(RefCell::new(Shared { a: None, is_callback_init: true, next_value: 42, received: None, freed: false, restart: None }));

	let shared_a = shared.clone();
	let a = Reinit::new0(move |restart| {
		let mut s = shared_a.deref().borrow_mut();
		s.restart = Some(restart);
		s.next_value
	});
	shared.deref().borrow_mut().a = Some(a.clone());
	assert!(matches!(a.test_get_state(), State::Initialized));

	// add callback
	a.add_callback(&shared, |shared, v| {
		let mut s = shared.deref().borrow_mut();
		assert!(matches!(s.a.as_ref().unwrap().test_get_state(), State::Initialized));
		assert_eq!(s.received, None);
		s.received = Some(*v);
	}, |shared| {
		let mut s = shared.deref().borrow_mut();
		assert!(matches!(s.a.as_ref().unwrap().test_get_state(), State::Destructing));
		assert!(!s.freed);
		assert_eq!(s.received, None, "must not give value and then clear it");
		s.freed = true;
	});
	{
		let mut s = shared.deref().borrow_mut();
		s.is_callback_init = false;
		assert_eq!(s.received, Some(42));
		assert!(!s.freed);
	}

	// restart
	let restart;
	{
		let mut s = shared.deref().borrow_mut();
		s.next_value = 127;
		s.received = None;
		restart = s.restart.as_ref().unwrap().clone();
	}
	restart.restart();
	{
		let s = shared.deref().borrow_mut();
		assert!(matches!(a.test_get_state(), State::Initialized));
		assert!(s.freed);
		assert_eq!(s.received, Some(127));
	}
}

#[test]
fn reinit0_restart() {
	type A = AT<()>;

	// init
	let shared = Shared::new();
	let a = A::new(&shared, ());
	assert_eq!(*shared.deref().borrow_mut(), Shared {
		a: Calls {
			new: 1,
			callback: 2,
			..Default::default()
		},
		counter: 3,
		..Default::default()
	});
	shared.deref().borrow_mut().reset();

	// restart A
	a.test_restart();
	assert_eq!(*shared.deref().borrow_mut(), Shared {
		a: Calls {
			callback_drop: 1,
			drop: 2,
			new: 3,
			callback: 4,
		},
		counter: 5,
		..Default::default()
	});
	shared.deref().borrow_mut().reset();
}

// TODO: need proper shutdown mechanism, ignored for now
#[test]
#[ignore]
fn reinit0_free() {
	let freed = Arc::new(Cell::new(false));
	let freed2 = freed.clone();

	struct Freeable {
		freed: Arc<Cell<bool>>,
	}

	impl Drop for Freeable {
		fn drop(&mut self) {
			self.freed.set(true);
		}
	}

	{
		let _reinit = Reinit::new0(move |_| {
			Freeable {
				freed: freed2.clone()
			}
		});
		assert!(!freed.get());
	}
	assert!(freed.get(), "Leaked Arc");
}

#[test]
fn reinit1_basic() {
	type A = AT<i32>;
	type B = BT<i32, A>;

	let shared = Shared::new();
	let a = A::new(&shared, 0);
	assert!(matches!(a.test_get_state(), State::Initialized));
	let b = B::new(&shared, &a, 0);
	assert!(matches!(b.test_get_state(), State::Initialized));

	assert_eq!(a.test_get_instance() as *const A, b.test_get_instance().a.deref() as *const A);
	assert_eq!(a.test_get_instance().t, 0);
	a.test_get_instance().t = 42;
	assert_eq!(b.test_get_instance().a.t, 42);
	assert_eq!(b.test_get_instance().t, 0);
}

#[test]
fn reinit1_restart() {
	type A = AT<()>;
	type B = BT<(), A>;

	// init
	let shared = Shared::new();
	let a = A::new(&shared, ());
	let _b = B::new(&shared, &a, ());
	assert_eq!(*shared.deref().borrow_mut(), Shared {
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
	shared.deref().borrow_mut().reset();

	// restart
	a.test_restart();
	assert_eq!(*shared.deref().borrow_mut(), Shared {
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
	shared.deref().borrow_mut().reset();
}

#[test]
fn reinit2_diamond_b_then_c() {
	reinit2_diamond(true);
}

#[test]
fn reinit2_diamond_c_then_b() {
	reinit2_diamond(false);
}

fn reinit2_diamond(b_then_c: bool) {
	type A = AT<()>;
	type B = BT<(), A>;
	type C = CT<(), A>;
	type D = DT<(), B, C>;

	// init
	let shared = Shared::new();
	let a = A::new(&shared, ());
	let b;
	let c;
	if b_then_c {
		b = B::new(&shared, &a, ());
		c = C::new(&shared, &a, ());
	} else {
		c = C::new(&shared, &a, ());
		b = B::new(&shared, &a, ());
	}
	let _d = D::new(&shared, &b, &c, ());

	{
		let mut expected = Shared {
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
		if !b_then_c {
			swap(&mut expected.b, &mut expected.c);
		}
		assert_eq!(*shared.deref().borrow_mut(), expected);
		shared.deref().borrow_mut().reset();
	}

	// restart
	{
		a.test_restart();
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
		assert_eq!(*shared.deref().borrow_mut(), expected);
		shared.deref().borrow_mut().reset();
	}
}

#[test]
fn reinit_restart_during_construction() {
	type A = AT<i32>;

	struct State {
		counter: AtomicI32,
		// oof, inefficient, unnecessary Arc
		shared: SharedRef,
		shared_during_restart: RefCell<Shared>,
		shared_before_restart: RefCell<Shared>,
	}

	// copy from AT::new and modified
	fn new(shared: &Arc<State>) -> Reinit<A> {
		let shared_a = shared.clone();
		let a = Reinit::new0(move |restart| {
			shared_a.shared.borrow_mut().register(|s| &mut s.a.new);
			let i = shared_a.counter.fetch_add(1, Relaxed);
			match i {
				1 => {
					*shared_a.shared_before_restart.borrow_mut() = *shared_a.shared.borrow_mut();
					shared_a.shared.borrow_mut().reset();
					restart.restart();
				}
				2 => {
					*shared_a.shared_during_restart.borrow_mut() = *shared_a.shared.borrow_mut();
					shared_a.shared.borrow_mut().reset();
				}
				_ => {}
			}
			AT { shared: shared_a.shared.clone(), t: i }
		});
		a.add_callback(
			shared,
			|shared, _a| shared.shared.borrow_mut().register(|s| &mut s.a.callback),
			|shared| shared.shared.borrow_mut().register(|s| &mut s.a.callback_drop),
		);
		a
	}

	let shared = Arc::new(State {
		counter: AtomicI32::new(0),
		shared: Shared::new(),
		shared_before_restart: RefCell::new(Default::default()),
		shared_during_restart: RefCell::new(Default::default()),
	});
	let a = new(&shared);

	assert_eq!(shared.counter.load(Relaxed), 1);
	assert_eq!(a.test_get_instance().t, shared.counter.load(Relaxed) - 1);
	assert_eq!(*shared.shared.borrow_mut(), Shared {
		a: Calls {
			new: 1,
			callback: 2,
			..Default::default()
		},
		counter: 3,
		..Default::default()
	});
	assert_eq!(*shared.shared_before_restart.borrow_mut(), Default::default());
	assert_eq!(*shared.shared_during_restart.borrow_mut(), Default::default());
	shared.shared.borrow_mut().reset();

	a.test_restart();
	assert_eq!(shared.counter.load(Relaxed), 3);
	assert_eq!(a.test_get_instance().t, shared.counter.load(Relaxed) - 1);
	assert_eq!(*shared.shared_before_restart.borrow_mut(), Shared {
		a: Calls {
			callback_drop: 1,
			drop: 2,
			new: 3,
			callback: 0,
		},
		counter: 4,
		..Default::default()
	});
	assert_eq!(*shared.shared_during_restart.borrow_mut(), Shared {
		a: Calls {
			callback_drop: 2,
			drop: 3,
			new: 4,
			callback: 1,
		},
		counter: 5,
		..Default::default()
	});
	assert_eq!(*shared.shared.borrow_mut(), Shared {
		a: Calls {
			callback_drop: 0,
			drop: 0,
			new: 0,
			callback: 1,
		},
		counter: 2,
		..Default::default()
	});
	shared.shared.borrow_mut().reset();
}

#[test]
fn reinit_no_restart_basic() {
	let inited = Arc::new(Cell::new(false));
	let inited2 = inited.clone();
	let reinit = Reinit::new_no_restart(move || {
		inited2.set(true);
	});
	assert!(matches!(reinit.test_get_state(), State::Initialized));
	assert_eq!(reinit.ptr.countdown.load(Relaxed), 0);
	assert!(inited.get());
	assert!(!reinit.ptr.queued_restart.load(Relaxed));
	assert!(reinit.ptr.ref_cnt.load(Relaxed) > 0);
}

#[test]
#[should_panic(expected = "Constructed more than once!")]
fn reinit_no_restart_restarted() {
	let reinit = Reinit::new_no_restart(move || {
		Arc::new(42)
	});
	reinit.test_restart();
}
