use std::fmt::{Debug, Display};
use std::sync::Arc;

pub trait Bla {
	fn work(&self);
}

pub trait Gen<T>: Debug {
	fn gen(&self) -> T;
}

#[derive(Debug)]
pub struct Test {
	cnt: u32,
}

impl Bla for Test {
	fn work(&self) {}
}

impl Gen<u32> for Test {
	fn gen(&self) -> u32 {
		self.cnt
	}
}

fn main() {
	{
		let b = Box::new(Test { cnt: 42 });
		let a = b as Box<dyn Gen<u32>>;
		println!("{:#?}", a);
	}

	{
		let b = Arc::new(Test { cnt: 42 });
		let a = b as Arc<dyn Gen<u32>>;
		println!("{:#?}", a);
	}

	// {
	// 	let b = Test { cnt: 42 };
	// 	let a: dyn Gen<u32> = b as dyn Gen<u32>;
	// 	println!("{:#?}", a);
	// }

	// let arc = Arc::new(Test { cnt: 42 });
	// call_work(&arc);
	// call_work(&(*&arc as Arc<dyn Bla>));
	// call_work(&Arc::downcast::<dyn Bla>(arc));

	// add_callback(&(arc as Arc<dyn Gen<u32>>));
}

fn call_work(bla: &Arc<dyn Bla>) {
	bla.work();
}

pub fn add_callback(callback: &Arc<dyn Gen<u32>>) -> u32 {
	callback.gen()
}
