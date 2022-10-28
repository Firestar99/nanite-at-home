use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

use crate::reinit::{Reinit, ReinitRef, Restart, State};

#[derive(Default, Eq, PartialEq, Debug)]
struct Calls {
	new: i32,
	drop: i32,
	callback: i32,
	callback_drop: i32,
}

#[derive(Eq, PartialEq, Debug)]
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
