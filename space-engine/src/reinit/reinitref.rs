use std::ops::Deref;
use crate::reinit::internal::Reinit;

#[repr(transparent)]
pub struct ReinitRef<T> {
	parent: Reinit<T>,
}

impl<T> ReinitRef<T> {
	#[inline]
	fn new(parent: &Reinit<T>) -> Self {
		parent.ref_inc();
		Self {
			parent
		}
	}
}

impl<T> Clone for ReinitRef<T> {
	#[inline]
	fn clone(&self) -> Self {
		ReinitRef::new(self.parent)
	}
}

impl<'a, T> Drop for ReinitRef<T> {
	#[inline]
	fn drop(&mut self) {
		self.parent.ref_dec();
	}
}

impl<T> Deref for ReinitRef<T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		unsafe {
			// SAFETY: t should exist as ReinitRef ref counts for t
			(&*self.parent.t.get()).assume_init_ref()
		}
	}
}