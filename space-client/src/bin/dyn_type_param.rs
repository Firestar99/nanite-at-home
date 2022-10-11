use std::fmt::Debug;
use std::mem::size_of;

#[derive(Debug)]
struct Outer {
	inner: Box<Inner>,
}

trait Bla: Debug + 'static {
	fn work(&self);
}

#[derive(Debug)]
struct Inner {
	count: u32,
	generic: Box<dyn Bla>,
}

#[derive(Debug)]
struct BlaImpl {
	float: f32,
}

impl Bla for BlaImpl {
	fn work(&self) {
		println!("{:#?}", self);
	}
}

fn main() {
	let inner = Inner {
		count: 42,
		generic: Box::new(BlaImpl {
			float: 0.2,
		}),
	};
	println!("{:#?}", inner);

	let inner = Box::new(inner);
	println!("{:#?}", inner);

	let outer = Outer {
		inner
	};

	println!("{:#?}", outer);
	println!("{}", size_of::<Outer>());
}