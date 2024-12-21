use crate::backing::table::RcTableSlot;
use crate::descriptor::{DescContentCpu, ImageSlot, RCDesc, RCDescExt};
use crate::descriptor::{MutDesc, MutDescExt};
use crate::pipeline::access_lock::AccessLockError;
use crate::pipeline::access_type::{ImageAccess, ImageAccessType, ShaderReadWriteable, ShaderReadable};
use crate::platform::{BindlessPipelinePlatform, RecordingContext, RecordingResourceContext};
use rust_gpu_bindless_shaders::descriptor::{Image, ImageType, MutImage, TransientDesc};
use std::marker::PhantomData;

pub trait MutImageAccessExt<P: BindlessPipelinePlatform, T: ImageType>: MutDescExt<P, MutImage<T>> {
	/// Access this mutable image to use it for recording.
	fn access<'a, A: ImageAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutImageAccess<'a, P, T, A>, AccessLockError>;

	/// Access this mutable buffer to use it for recording. Discards the contents of this buffer and as if it were
	/// uninitialized.
	///
	/// # Safety
	/// Must not read uninitialized memory and fully overwrite it within this execution context.
	unsafe fn access_undefined_contents<'a, A: ImageAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutImageAccess<'a, P, T, A>, AccessLockError>;
}

impl<P: BindlessPipelinePlatform, T: ImageType> MutImageAccessExt<P, T> for MutDesc<P, MutImage<T>> {
	fn access<'a, A: ImageAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutImageAccess<'a, P, T, A>, AccessLockError> {
		MutImageAccess::from(self, cmd)
	}

	unsafe fn access_undefined_contents<'a, A: ImageAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutImageAccess<'a, P, T, A>, AccessLockError> {
		MutImageAccess::from_undefined_contents(self, cmd)
	}
}

pub struct MutImageAccess<'a, P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType> {
	slot: RcTableSlot,
	resource_context: &'a P::RecordingResourceContext,
	_phantom: PhantomData<T>,
	_phantom2: PhantomData<A>,
}

impl<'a, P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType> MutImageAccess<'a, P, T, A> {
	pub fn from_undefined_contents(
		desc: MutDesc<P, MutImage<T>>,
		cmd: &P::RecordingContext<'a>,
	) -> Result<Self, AccessLockError> {
		Self::from_inner(desc, cmd, |_| ImageAccess::Undefined)
	}

	pub fn from(desc: MutDesc<P, MutImage<T>>, cmd: &P::RecordingContext<'a>) -> Result<Self, AccessLockError> {
		Self::from_inner(desc, cmd, |x| x)
	}

	#[inline]
	fn from_inner(
		desc: MutDesc<P, MutImage<T>>,
		cmd: &P::RecordingContext<'a>,
		f: impl FnOnce(ImageAccess) -> ImageAccess,
	) -> Result<Self, AccessLockError> {
		unsafe {
			let this = Self {
				slot: desc.into_rc_slot(),
				resource_context: cmd.resource_context(),
				_phantom: PhantomData,
				_phantom2: PhantomData,
			};
			this.transition_inner(f(this.inner_slot().access_lock.try_lock()?), A::IMAGE_ACCESS);
			Ok(this)
		}
	}

	/// Transition this Image from one [`ImageAccessType`] to another and inserts appropriate barriers.
	pub fn transition<B: ImageAccessType>(self) -> MutImageAccess<'a, P, T, B> {
		self.transition_inner(A::IMAGE_ACCESS, B::IMAGE_ACCESS);
		MutImageAccess {
			slot: self.slot,
			resource_context: self.resource_context,
			_phantom: PhantomData,
			_phantom2: PhantomData,
		}
	}

	#[inline]
	fn transition_inner(&self, src: ImageAccess, dst: ImageAccess) {
		if src != dst {
			unsafe { self.resource_context.transition_image(&self.inner_slot(), src, dst) }
		}
	}

	#[inline]
	pub unsafe fn inner_slot(&self) -> &ImageSlot<P> {
		MutImage::<T>::get_slot(&self.slot)
	}

	/// Turns this mutable access to a [`MutImage`] back into a [`MutImage`] to be used in another execution
	pub fn into_desc(self) -> MutDesc<P, MutImage<T>> {
		unsafe {
			self.inner_slot().access_lock.unlock(A::IMAGE_ACCESS);
			MutDesc::new(self.slot)
		}
	}

	/// Turns this mutable access to a [`MutImage`] into a shared [`RCDesc`]
	pub fn into_shared(self) -> RCDesc<P, Image<T>> {
		unsafe {
			self.transition_inner(A::IMAGE_ACCESS, ImageAccess::GeneralRead);
			self.inner_slot().access_lock.unlock_to_shared();
			RCDesc::new(self.slot)
		}
	}
}

impl<'a, P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType + ShaderReadable> MutImageAccess<'a, P, T, A> {
	pub fn to_transient(&self) -> TransientDesc<Image<T>> {
		// Safety: mutable resource is in a layout that implements ShaderReadable, so it is readable by a shader
		unsafe { TransientDesc::new(self.slot.id(), &self.resource_context.to_transient_access()) }
	}
}

impl<'a, P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType + ShaderReadWriteable>
	MutImageAccess<'a, P, T, A>
{
	pub fn to_mut_transient(&self) -> TransientDesc<MutImage<T>> {
		// Safety: mutable resource is in a layout that implements ShaderReadWriteable, so it is readable and writeable
		// by a shader
		unsafe { TransientDesc::new(self.slot.id(), &self.resource_context.to_transient_access()) }
	}
}
