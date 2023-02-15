use crate::reinit::{Reinit, ReinitDetails};

pub trait Target {}

pub struct NeedGuard<T: Target + 'static, D: ReinitDetails<T, D>> {
	reinit: &'static Reinit<T, D>,
}

impl<T: Target + 'static, D: ReinitDetails<T, D>> Reinit<T, D> {
	pub fn need(&'static self) -> NeedGuard<T, D> {
		NeedGuard::new(self)
	}
}

impl<T: Target + 'static, D: ReinitDetails<T, D>> NeedGuard<T, D> {
	fn new(reinit: &'static Reinit<T, D>) -> Self {
		unsafe { reinit.need_inc() }
		Self { reinit }
	}
}

impl<T: Target + 'static, D: ReinitDetails<T, D>> Drop for NeedGuard<T, D> {
	fn drop(&mut self) {
		unsafe { self.reinit.need_dec() }
	}
}
