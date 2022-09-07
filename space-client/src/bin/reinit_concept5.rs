use std::cell::Cell;
use std::ops::Deref;
use std::rc::{Rc, Weak};

pub trait Re<'a, T> {
	fn subscribe(&self, callback: Weak<dyn Callback<'a, T>>);
}

pub trait Callback<'a, T> {
	fn accept(&self, t: Rc<T>);

	fn request_drop(&self);
}

struct DropRedirect<T> {

	reinit: Reinit<T>,
}

impl<T> Drop for DropRedirect<T> {
	fn drop(&mut self) {
		todo!()
	}
}

impl<T> Deref for DropRedirect<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		todo!()
	}
}

pub struct Reinit<T>
{
	t: Cell<Option<T>>,
}

impl<T> Reinit<T> {
	fn new() -> Self {
		Self {
			t: Cell::new(None),
		}
	}
}

impl<T> Deref for Reinit<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe {
			// TODO is this really safe??
			self.t.as_ptr().as_ref()
		}.unwrap().as_ref().unwrap()
	}
}

pub struct Reinit0<T, F> {
	base: Reinit<T>,
	constructor: F,
}

impl<T, F> Reinit0<T, F>
	where
		F: Fn() -> T
{
	pub fn new(constructor: F) -> Box<Self> {
		let reinit = Box::new(Self {
			base: Reinit::new(),
			constructor,
		});
		reinit.reinit();
		reinit
	}

	pub fn reinit(&self) {
		self.base.t.set(Some((self.constructor)()));
	}
}

impl<T, F> Deref for Reinit0<T, F> {
	type Target = Reinit<T>;

	fn deref(&self) -> &Self::Target {
		&self.base
	}
}

pub struct Reinit1<'a, T, F, A> {
	base: Reinit<T>,
	constructor: F,
	a: &'a Reinit<A>,
}

impl<'a, T, F, A> Reinit1<'a, T, F, A>
	where
		F: Fn(&'a A) -> T
{
	pub fn new(a: &'a Reinit<A>, constructor: F) -> Box<Self> {
		let reinit = Box::new(Self {
			base: Reinit::new(),
			constructor,
			a,
		});
		reinit.reinit();
		reinit
	}

	pub fn reinit(&self) {
		self.base.t.set(Some((self.constructor)(self.a)));
	}
}

impl<T, F, A> Deref for Reinit1<'_, T, F, A> {
	type Target = Reinit<T>;

	fn deref(&self) -> &Self::Target {
		&self.base
	}
}

pub struct Reinit2<'a, T, F, A, B> {
	base: Reinit<T>,
	constructor: F,
	a: &'a Reinit<A>,
	b: &'a Reinit<B>,
}

impl<'a, T, F, A, B> Reinit2<'a, T, F, A, B>
	where
		F: Fn(&'a A, &'a B) -> T
{
	pub fn new(a: &'a Reinit<A>, b: &'a Reinit<B>, constructor: F) -> Box<Self> {
		let reinit = Box::new(Self {
			base: Reinit::new(),
			constructor,
			a,
			b,
		});
		reinit.reinit();
		reinit
	}

	pub fn reinit(&self) {
		self.base.t.set(Some((self.constructor)(self.a, self.b)));
	}
}

impl<T, F, A, B> Deref for Reinit2<'_, T, F, A, B> {
	type Target = Reinit<T>;

	fn deref(&self) -> &Self::Target {
		&self.base
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
	let a = Reinit0::new(|| A {});
	let b = Reinit1::new(&a, |a| B { a });
	let c = Reinit1::new(&a, |a| C { a });
	let c = Reinit2::new(&b, &c, |b, c| D { b, c });

	c.work();

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
