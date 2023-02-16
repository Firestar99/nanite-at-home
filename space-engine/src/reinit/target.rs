use crate::reinit::{Reinit, ReinitRef};

pub trait Target {}

pub struct NeedGuard<T: 'static> {
	reinit: &'static Reinit<T>,
}

impl<T: 'static> NeedGuard<T> {
	fn new(reinit: &'static Reinit<T>) -> Self {
		unsafe { reinit.need_inc() }
		Self { reinit }
	}
}

impl<T: 'static> Drop for NeedGuard<T> {
	fn drop(&mut self) {
		unsafe { self.reinit.need_dec() }
	}
}

impl<T: Target + 'static> Reinit<T> {
	pub fn need(&'static self) -> NeedGuard<T> {
		NeedGuard::new(self)
	}
}

#[cfg(test)]
impl<T: 'static> Reinit<T> {
	pub fn test_need(&'static self) -> NeedGuard<T> {
		NeedGuard::new(self)
	}
}

#[cfg(test)]
impl<T: 'static> NeedGuard<T> {
	pub fn test_ref(&'static self) -> ReinitRef<T> {
		self.reinit.test_ref()
	}
}
