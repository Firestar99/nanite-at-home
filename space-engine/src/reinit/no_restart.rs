use std::cell::UnsafeCell;

use crate::reinit::{Constructed, global_need_dec, global_need_inc, Reinit, ReinitDetails};

// ReinitNoRestart
#[allow(clippy::type_complexity)]
pub struct ReinitNoRestart<T: Send + Sync + 'static>
{
	constructor: UnsafeCell<Option<fn(Constructed<T>)>>,
}

// member constructor is not Sync
unsafe impl<T: Send + Sync + 'static> Sync for ReinitNoRestart<T> {}

impl<T: Send + Sync + 'static> ReinitNoRestart<T> {
	pub const fn new(constructor: fn(Constructed<T>)) -> Self
	{
		Self {
			constructor: UnsafeCell::new(Some(constructor))
		}
	}

	pub const fn create_reinit(&'static self) -> Reinit<T> {
		Reinit::new(0, self)
	}
}

#[macro_export]
macro_rules! reinit_no_restart_internal {
	($vis:vis $name:ident: $t:ty = $f:expr) => ($crate::paste::paste!{
		static [<$name _DETAILS>]: $crate::reinit::ReinitNoRestart<$t> = $crate::reinit::ReinitNoRestart::new($f);
		$vis static $name: $crate::reinit::Reinit<$t> = [<$name _DETAILS>].create_reinit();
	});
}

/// default reinit macro, ~~always delegates initialization to an async task~~
/// NOTE: the no_restart variant actually does NOT spawn a task and acts like [`reinit_no_restart_map!`]
#[macro_export]
macro_rules! reinit_no_restart {
	($vis:vis $name:ident: $t:ty = $f:expr) => {
		$crate::reinit_no_restart_map!($vis $name: $t = $f);
	};
}

/// reinit macro expecting a `Future<Output=T>`, always delegates initialization to an async task
#[macro_export]
macro_rules! reinit_no_restart_future {
	($vis:vis $name:ident: $t:ty = $f:expr) => {
		$crate::reinit_no_restart_internal!($vis $name: $t = |con: $crate::reinit::Constructed<$t>| $crate::spawn(async move { con.constructed($f.await) }).detach());
	};
}

/// reinit macro which does the initialization immediately instead of spawning a task, for small things such as just mapping a value
#[macro_export]
macro_rules! reinit_no_restart_map {
	($vis:vis $name:ident: $t:ty = $f:expr) => {
		$crate::reinit_no_restart_internal!($vis $name: $t = |con: $crate::reinit::Constructed<$t>| con.constructed($f));
	};
}

impl<T: Send + Sync + 'static> ReinitDetails<T> for ReinitNoRestart<T>
{
	fn init(&'static self, _: &'static Reinit<T>) {}

	unsafe fn on_need_inc(&'static self, _: &'static Reinit<T>) {
		global_need_inc()
	}

	unsafe fn on_need_dec(&'static self, _: &'static Reinit<T>) {
		global_need_dec()
	}

	fn request_construction(&'static self, parent: &'static Reinit<T>) {
		// SAFETY: this may not be atomic, but that's ok as Reinit will act as a Mutex for this method
		let constructor = unsafe { &mut *self.constructor.get() }.take();
		(constructor.expect("Constructed more than once!"))(Constructed(parent));
	}
}
