use space_engine::reinit::{Reinit, ReinitRef, Restart};

#[derive(Debug)]
pub struct A {
	pub count: i32,
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
	pub extra: String,
}

impl Drop for B {
	fn drop(&mut self) {
		println!("destroyed {:#?}", self);
	}
}

fn main() {
	let a = Reinit::new0(|restart| {
		let a = A { count: 42, restart };
		println!("constructed {:#?}", a);
		a
	});
	let b = Reinit::new1(a, |a, _| {
		let b = B { a, extra: String::from("test") };
		println!("constructed {:#?}", b);
		b
	});

	let main_state = Reinit::new0(|_| 0);

	println!("exit")
	// TODO no drops happen cause instance.arc does not get dropped
}