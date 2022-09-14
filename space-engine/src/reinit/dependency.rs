use std::sync::Arc;
use std::mem::MaybeUninit;
use crate::reinit::reinit::Reinit;

struct Dependency<'a, D> {
	reinit: &'a Arc<Reinit<D>>,
	value: MaybeUninit<&'a D>,
}

impl<'a, D> Dependency<'a, D> {
	pub(in crate::reinit) fn new(a: &'a Arc<Reinit<D>>) -> Self {
		Self {
			reinit: a,
			value: MaybeUninit::uninit(),
		}
	}
}
