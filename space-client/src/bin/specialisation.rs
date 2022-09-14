trait Abstract {
	fn message_specialisation(&self);
}

struct Base {
	specialisation: *const dyn Abstract,
}

impl Base {
	fn message_base(&self) {
		println!("received by Base");
		unsafe { &*self.specialisation }.message_specialisation();
	}
}

struct A {
	base: Base,
	// in practice: each subtype has different fields, some generic
}

impl A {
	fn call(&self) {
		println!("called specialization");
		self.base.message_base();
	}
}

impl Abstract for A {
	fn message_specialisation(&self) {
		println!("received by specialization");
	}
}

fn main() {
	let mut a = A {
		base: Base {
			specialisation: &NULLPTR,
		}
	};
	a.base.specialisation = &a as *const dyn Abstract;
	a.call();
}



// std::ptr:null() doesn't seem to work? Using this as an alternative
static NULLPTR: Nullptr = Nullptr {};

struct Nullptr {}

impl Abstract for Nullptr {
	fn message_specialisation(&self) {
		panic!()
	}
}
