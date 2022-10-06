use std::sync::Arc;
use std::mem::MaybeUninit;
use crate::reinit_old::reinit::{Reinit, ReinitRef};

pub struct Dependency<D> {
	reinit: Arc<Reinit<D>>,
	value: Option<ReinitRef<D>>,
}

impl<'a, D> Dependency<D> {
	pub(in crate::reinit_old) fn new(a: &Arc<Reinit<D>>) -> Self {
		Self {
			reinit: a.clone(),
			value: None,
		}
	}
}
