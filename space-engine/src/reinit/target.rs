use crate::reinit::Reinit;

pub trait Target: Send + Sync {}

pub struct NeedGuard<T: Target + 'static> {
	pub reinit: &'static Reinit<T>,
}

impl<T: Target + 'static> NeedGuard<T> {
	fn new(reinit: &'static Reinit<T>) -> Self {
		unsafe { reinit.need_inc() }
		Self { reinit }
	}
}

impl<T: Target + 'static> Drop for NeedGuard<T> {
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
mod test {
	use std::time::Duration;

	use crate::reinit::Reinit;
	use crate::reinit::State::{Initialized, Uninitialized};

	pub struct TestNeedGuard<T: Send + Sync + 'static> {
		reinit: &'static Reinit<T>,
		timeout: Duration,
	}

	impl<T: Send + Sync + 'static> TestNeedGuard<T> {
		fn new(reinit: &'static Reinit<T>, timeout: Duration) -> Self {
			unsafe { reinit.need_inc() };
			reinit.busy_wait_until_state(Initialized, timeout);
			Self { reinit, timeout }
		}

		/// drop and do NOT wait for Reinit to go into State Uninitialized
		pub fn drop_and_wait(self) {
			let reinit = self.reinit;
			let timeout = self.timeout;
			drop(self);
			reinit.busy_wait_until_state(Uninitialized, timeout);
		}
	}

	impl<T: Send + Sync + 'static> Drop for TestNeedGuard<T> {
		fn drop(&mut self) {
			unsafe { self.reinit.need_dec() }
		}
	}

	impl<T: Send + Sync + 'static> TestNeedGuard<T> {
		pub fn test_ref(&'static self) -> crate::reinit::ReinitRef<T> {
			self.reinit.test_ref()
		}
	}

	impl<T: Send + Sync + 'static> Reinit<T> {
		pub fn test_need(&'static self) -> TestNeedGuard<T> {
			self.test_need_timeout(Duration::from_secs(1))
		}

		pub fn test_need_timeout(&'static self, timeout: Duration) -> TestNeedGuard<T> {
			TestNeedGuard::new(self, timeout)
		}
	}
}
