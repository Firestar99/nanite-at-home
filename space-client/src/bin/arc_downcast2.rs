use std::sync::{Arc, Weak};

trait Callback<T> {
	fn call(&self, t: T);
}

#[derive(Debug)]
pub struct Bla {
	pub cnt: i32,
}

impl Callback<u32> for Bla {
	fn call(&self, t: u32) {
		println!("call({:#?}, {});", self, t);
	}
}

fn main() {
	let arc = Arc::new(Bla { cnt: 42 });
	let mut test = Test { myweak: None };
	test.call_ref(&arc);
}

struct Test<T> {
	myweak: Option<Weak<dyn Callback<T>>>,
}

impl<T> Test<T> {
	fn call_ref<C>(&mut self, arc: &Arc<C>)
		where
			C: Callback<T> + 'static
	{
		let weak = Arc::downgrade(arc);
		let weak2: Weak<dyn Callback<T>> = weak as Weak<dyn Callback<T>>;
		self.myweak = Some(weak2);
	}
}
