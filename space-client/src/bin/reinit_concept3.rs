use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

pub trait Re<'a, T> {
	fn subscribe(&self, callback: Weak<dyn Callback<'a, T>>);
}

pub trait Callback<'a, T> {
	fn accept(&self, t: Rc<T>);

	fn drop_ref(&self);
}

pub struct Reinit<'a, T, F, D>
{
	dependencies: D,
	constructor: F,
	t: Cell<Option<T>>,
	_phantom: PhantomData<&'a ()>,
}

impl<'a, T, F> Reinit<'a, T, F, ()>
	where
		F: Fn() -> T
{
	pub fn new(constructor: F) -> Rc<Self> {
		let reinit = Rc::new(Self {
			dependencies: (),
			constructor,
			t: Cell::new(None),
			_phantom: Default::default(),
		});
		reinit.reinit();
		reinit
	}

	pub fn reinit(&self) {
		self.t.set(Some((self.constructor)()));
	}
}

struct DependencyEntry<'a, A> {
	value: Option<&'a A>,

}

impl<'a, T, F, A> Reinit<'a, T, F, (&'a A, )>
	where
		F: Fn(&'a A) -> T
{
	pub fn new(a: &'a A, constructor: F) -> Self {
		let reinit = Self {
			dependencies: (a, ),
			constructor,
			t: Cell::new(None),
			_phantom: Default::default(),
		};
		reinit.reinit();
		reinit
	}

	pub fn reinit(&self) {
		self.t.set(Some((self.constructor)(self.dependencies.0)));
	}
}


struct A {}

struct B<'a> {
	a: &'a A,
}

struct C<'a> {
	a: &'a A,
}

struct D<'a> {
	b: &'a B<'a>,
	c: &'a C<'a>,
}

impl<'a> D<'a> {
	fn work(&self) {}
}

pub fn main() {
	// let a = Reinit::new(|| A {});

// 	let mut exit = false;
// 	while !exit {
// 		let mut a = Reinit::new(A {});
// 		while !exit && !a.should() {
// 			let mut b = Reinit::new(B { a: &a.t });
// 			while !exit && !b.should() && !a.should() {
// 				let mut c = Reinit::new(C { a: &a.t });
// 				while !exit && !c.should() && !b.should() && !a.should() {
// 					let mut d = Reinit::new(D { b: &b.t, c: &c.t });
// 					while !exit && !d.should() && !c.should() && !b.should() && !a.should() {
// 						d.work();
//
// 						if restart_a() {
// 							a.restart();
// 						}
// 						if restart_b() {
// 							b.restart();
// 						}
// 						if restart_c() {
// 							c.restart();
// 						}
// 						if restart_d() {
// 							d.restart();
// 						}
// 						if should_exit() {
// 							exit = true;
// 						}
// 					}
// 				}
// 			}
// 		}
// 	}
}

fn restart_a() -> bool {
	false
}

fn restart_b() -> bool {
	false
}

fn restart_c() -> bool {
	false
}

fn restart_d() -> bool {
	false
}

fn should_exit() -> bool {
	false
}
