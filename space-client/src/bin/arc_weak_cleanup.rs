use std::sync::{Arc, Weak};

trait Callback {
	fn call(&self);
}

struct Bla {}

impl Callback for Bla {
	fn call(&self) {
		println!("call!");
	}
}

fn main() {
	let mut callbacks: Vec<Weak<dyn Callback>> = vec![];
	{
		let arc = Arc::new(Bla {});
		// FIRST: how can I downcast Arc<Bla> to Arc<dyn Callback>? Arc's T is unsized so it should be possible.
		callbacks.push(Arc::<Bla>::downgrade(&arc));
		let weak = Arc::downgrade(&arc);
		callbacks.push(weak);
		// arc drops
	}

	// for x in callbacks.iter_mut() {
	// 	match x.upgrade() {
	// 		// weak fails to upgrade, cause arc was dropped -> None branch
	// 		None => {
	// 			// optimization: drop Weak to free memory used by Arc
	// 			// Arc drops value when strong refs = 0, but only deallocates memory when all weak refs = 0
	// 			drop(*x);
	// 			// *x = Weak::<dyn Callback>::new()
	// 			// SECOND: I cannot call Weak::new() due to T being unsized
	// 			// idea 1: remove values from vec, but that would require unnecessary moving of values
	// 			// idea 2: have Vec<Optional<Weak<_>>> and replace with None, but unsure if "null pointer optimization" works (see Option #Representation)
	// 		}
	// 		Some(x) => { x.call() }
	// 	}
	// }
}
