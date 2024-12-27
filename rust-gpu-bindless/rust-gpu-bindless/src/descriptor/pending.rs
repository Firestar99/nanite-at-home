use crate::platform::{BindlessPlatform, PendingExecution};

pub struct Pending<P: BindlessPlatform, T> {
	pending: P::PendingExecution,
	t: T,
}

impl<P: BindlessPlatform, T> Pending<P, T> {
	pub fn completed(t: T) -> Pending<P, T> {
		Self {
			pending: PendingExecution::<P>::completed(),
			t,
		}
	}

	pub fn block_on(self) -> T {
		self.pending.block_on();
		self.t
	}
}
