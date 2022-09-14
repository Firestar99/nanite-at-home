use std::sync::Arc;

trait BlaTrait {
	fn work(&self);
}

struct StructA {

}

impl BlaTrait for StructA {
	fn work(&self) {
		println!("work A");
	}
}

impl Drop for StructA {
	fn drop(&mut self) {
		println!("dropped A");
	}
}

struct StructB {

}

impl BlaTrait for StructB {
	fn work(&self) {
		println!("work B");
	}
}

impl Drop for StructB {
	fn drop(&mut self) {
		println!("dropped B");
	}
}

fn main() {
	let arc = Arc::new(StructA {});
	let arc1 = arc.clone() as Arc<dyn BlaTrait>;
	drop(arc);

	arc1.work();
	drop(arc1);
}
