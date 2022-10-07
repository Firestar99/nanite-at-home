pub trait Callback<T> {
	fn accept(&self, t: ReinitRef<T>);

	fn request_drop(&self);
}

mod internal;
pub mod reinitref;
pub mod reinit1;