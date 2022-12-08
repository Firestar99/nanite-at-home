use std::cell::Cell;

use space_engine::reinit::{Reinit, ReinitRef, Restart};

#[derive(Debug)]
pub struct A {
	pub count: Cell<i32>,
	pub restart: Restart<A>,
}

impl Drop for A {
	fn drop(&mut self) {
		println!("destroyed {:#?}", self);
	}
}

#[derive(Debug)]
pub struct B {
	pub a: ReinitRef<A>,
	pub restart: Restart<B>,
	pub extra: String,
}

impl Drop for B {
	fn drop(&mut self) {
		println!("destroyed {:#?}", self);
	}
}

#[derive(Debug)]
pub struct C {
	pub a: ReinitRef<A>,
	pub restart: Restart<C>,
}

impl Drop for C {
	fn drop(&mut self) {
		println!("destroyed {:#?}", self);
	}
}

#[derive(Debug)]
pub struct D {
	pub b: ReinitRef<B>,
	pub c: ReinitRef<C>,
	pub restart: Restart<D>,
}

impl Drop for D {
	fn drop(&mut self) {
		println!("destroyed {:#?}", self);
	}
}

#[allow(unused_variables)]
fn main() {
	let a = Reinit::new0(|restart| {
		let a = A { count: Cell::new(42), restart };
		println!("constructed {:#?}", a);
		a
	});
	let b = Reinit::new1(&a, |a, restart| {
		let b = B { a, extra: String::from("test"), restart };
		println!("constructed {:#?}", b);
		b
	});
	let c = Reinit::new1(&a, |a, restart| {
		let c = C { a, restart };
		println!("constructed {:#?}", c);
		c
	});
	let d = Reinit::new2(&b, &c, |b, c, restart| {
		let d = D { b, c, restart };
		println!("constructed {:#?}", d);
		d
	});

	let main_state = Reinit::new0(|_| Cell::new(0));
	let main = Reinit::new2(&a, &main_state, |a, state, _| {
		match state.get() {
			0 => {
				println!("restart A");
				a.count.set(9);

				// condition: Reinit does not keep ReinitRef beyond construction

				// main get's constructed
				// restarts A
				// A requests main to drop
				// main records request to uninit, but cannot destruct as it's currently constructing
				// this closure holds a reference to ReinitRef<A>
				// this closure returns, drops ReinitRef, allows A to restart
				// A gives main the new A value
				// main will increment countdown, erasing any information about the required reboot
				// BUG main does not construct again
				a.restart.restart();
			}
			1 => println!("done!"),
			_ => {}
		};
		state.set(state.get() + 1);
		// a
	});

	println!("exit")
	// TODO no drops happen cause instance.arc does not get dropped
}