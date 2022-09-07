use std::any::Any;

// struct Arena<const CAP: usize> {
// 	dependency_table: [[u32; 32]; CAP],
// 	values: [Box<dyn EntryTrait>; CAP],
//
// }
//
// // trait EntryTrait {
// // 	fn bla<'a>(&self, arena: &'a Arena);
// // }
//
// struct Entry<T, F> {
// 	constructor: F,
// 	t: T,
// }


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
	// let a = Reinit::new(|| A {});

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
