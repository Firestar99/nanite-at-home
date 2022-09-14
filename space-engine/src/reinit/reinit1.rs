use std::sync::Arc;
use crate::reinit::reinit::{Callback, Dependency, Reinit, ReinitImpl, ReinitRef};

struct Reinit1<'a, T, F, A>
	where
		F: Fn(&A) -> T
{
	reinit: Reinit<T>,
	constructor: F,
	a: Dependency<'a, A>,
}

impl<'a, T, F, A> Reinit1<'a, T, F, A>
	where
		F: Fn(&A) -> T
{
	pub fn new(a: &'a Arc<Reinit<A>>, constructor: F) -> Arc<Self> {
		let mut arc = Arc::new(Self {
			reinit: Reinit::new(),
			constructor,
			a: Dependency::new(a),
		});
		arc.reinit.init(&*arc);
		// a.add_callback(&arc);
		arc
		//TODO do callback registration
	}
}

impl<'a, T, F, A> ReinitImpl<A> for Reinit1<'a, T, F, A> {

}


impl<'a, T, F, A> Callback<A> for Reinit1<'a, T, F, A>
	where
		F: Fn(&A) -> T
{
	fn accept(&self, t: ReinitRef<A>) {
		todo!()
	}

	fn request_drop(&self) {
		todo!()
	}
}
