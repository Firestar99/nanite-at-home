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
	let mut exit = false;
	while !exit {
		let a = A {};
		let mut reinit_a = false;
		while !exit && !reinit_a {
			let b = B { a: &a };
			let mut reinit_b = false;
			while !exit && !reinit_b && !reinit_a {
				let c = C { a: &a };
				let mut reinit_c = false;
				while !exit && !reinit_c && !reinit_b && !reinit_a {
					let d = D { b: &b, c: &c };
					let mut reinit_d = false;
					while !exit && !reinit_d && !reinit_c && !reinit_b && !reinit_a {
						d.work();

						if restart_a() {
							reinit_a = true;
						}
						if restart_b() {
							reinit_b = true;
						}
						if restart_c() {
							reinit_c = true;
						}
						if restart_d() {
							reinit_d = true;
						}
						if should_exit() {
							exit = true;
						}
					}
				}
			}
		}
	}
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
