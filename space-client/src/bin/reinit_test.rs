use space_engine::reinit::{Reinit, ReinitRef};

#[derive(Debug)]
pub struct A {
	pub count: i32,
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
	let a = Reinit::new0(|| {
		let a = A { count: 42 };
		println!("constructed {:#?}", a);
		a
	});
	let _b = Reinit::new1(a, |a| {
		let b = B { a, extra: String::from("test") };
		println!("constructed {:#?}", b);
		b
	});
	println!("work");
	// TODO no drops happen cause instance.arc does not get dropped
}