use std::sync::{Arc, Weak};

trait MyTrait {
	fn work(&self);
}

#[derive(Debug)]
struct TraitImpl {
	count: u32,
}

impl MyTrait for TraitImpl {
	fn work(&self) {
		println!("{:#?}", self);
	}
}

struct Wrapper {
	some_other_vars: String,
	trait_impl: TraitImpl,
}

fn consume(weak: &Weak<dyn MyTrait>) {
	if let Some(my_trait) = weak.upgrade() {
		my_trait.work();
	}
}

fn main() {
	let trait_impl = Arc::new(TraitImpl {
		count: 42
	});
	let trait_impl_weak = Arc::downgrade(&trait_impl) as Weak<dyn MyTrait>;
	consume(&trait_impl_weak);

	let wrapper = Arc::new(Wrapper {
		some_other_vars: String::from("ignore these"),
		trait_impl: TraitImpl {
			count: 122,
		},
	});
	// let wrapper_weak = Arc::downgrade(&wrapper) as Weak<dyn MyTrait>;
	// consume(wrapper_weak);
}

struct Bla<W> {
	weak: Weak<W>,
	ptr: *const dyn MyTrait,
}

impl<W> MyTrait for Bla<W> {
	fn work(&self) {
		if let Some(my_trait) = self.weak.upgrade() {
			unsafe { &*self.ptr }.work();
		}
	}
}
