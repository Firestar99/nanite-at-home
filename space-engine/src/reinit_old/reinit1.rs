use std::sync::Arc;

use crate::reinit_old::dependency::Dependency;
use crate::reinit_old::reinit::{Callback, Reinit, ReinitImpl, ReinitRef};

struct Reinit1<T, F, A>
	where
		F: Fn(&A) -> T
{
	reinit: Reinit<T>,
	constructor: F,
	a: Dependency<A>,
}

impl<T, F, A> Reinit1<T, F, A>
	where
		F: Fn(&A) -> T
{
	pub fn new(a: Arc<Reinit<A>>, constructor: F) -> Arc<Self> {
		let mut arc = Arc::new(Self {
			reinit: Reinit::new(),
			constructor,
			a: Dependency::new(a),
		});
		//TODO do callback registration
		// arc.reinit.init(&*arc);
		// a.add_callback(&arc);
		arc
	}
}

impl<'a, T, F, A> ReinitImpl for Reinit1<'a, T, F, A>
	where
		F: Fn(&A) -> T
{
	fn request_drop(&self) {
		todo!()
	}
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
