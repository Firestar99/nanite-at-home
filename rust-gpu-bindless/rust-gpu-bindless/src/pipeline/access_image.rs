use crate::backing::table::RcTableSlot;
use crate::descriptor::{BindlessImageUsage, DescTable, ImageSlot, ImageTable, RCDesc, RCDescExt};
use crate::descriptor::{MutDesc, MutDescExt};
use crate::pipeline::access_error::AccessError;
use crate::pipeline::access_type::{
	ImageAccess, ImageAccessType, ShaderReadWriteable, ShaderReadable, ShaderSampleable,
};
use crate::platform::{BindlessPipelinePlatform, RecordingContext, RecordingResourceContext};
use rust_gpu_bindless_shaders::descriptor::{Image, ImageType, MutImage, TransientDesc};
use std::marker::PhantomData;

pub trait MutImageAccessExt<P: BindlessPipelinePlatform, T: ImageType>: MutDescExt<P, MutImage<T>> {
	/// Access this mutable image to use it for recording. Panics if an [`AccessError`] occurred.
	fn access<'a, A: ImageAccessType>(self, cmd: &P::RecordingContext<'a>) -> MutImageAccess<'a, P, T, A> {
		self.try_access(cmd).unwrap()
	}

	/// Access this mutable image to use it for recording.
	fn try_access<'a, A: ImageAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutImageAccess<'a, P, T, A>, AccessError>;

	/// Access this mutable buffer to use it for recording. Discards the contents of this buffer and as if it were
	/// uninitialized. Panics if an [`AccessError`] occurred.
	///
	/// # Safety
	/// Must not read uninitialized memory and fully overwrite it within this execution context.
	unsafe fn access_undefined_contents<'a, A: ImageAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> MutImageAccess<'a, P, T, A> {
		self.try_access_undefined_contents(cmd).unwrap()
	}

	/// Access this mutable buffer to use it for recording. Discards the contents of this buffer and as if it were
	/// uninitialized.
	///
	/// # Safety
	/// Must not read uninitialized memory and fully overwrite it within this execution context.
	unsafe fn try_access_undefined_contents<'a, A: ImageAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutImageAccess<'a, P, T, A>, AccessError>;
}

impl<P: BindlessPipelinePlatform, T: ImageType> MutImageAccessExt<P, T> for MutDesc<P, MutImage<T>> {
	fn try_access<'a, A: ImageAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutImageAccess<'a, P, T, A>, AccessError> {
		MutImageAccess::from(self, cmd)
	}

	unsafe fn try_access_undefined_contents<'a, A: ImageAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutImageAccess<'a, P, T, A>, AccessError> {
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
	) -> Result<Self, AccessError> {
		Self::from_inner(desc, cmd, |_| ImageAccess::Undefined)
	}

	pub fn from(desc: MutDesc<P, MutImage<T>>, cmd: &P::RecordingContext<'a>) -> Result<Self, AccessError> {
		Self::from_inner(desc, cmd, |x| x)
	}

	#[inline]
	fn from_inner(
		desc: MutDesc<P, MutImage<T>>,
		cmd: &P::RecordingContext<'a>,
		f: impl FnOnce(ImageAccess) -> ImageAccess,
	) -> Result<Self, AccessError> {
		unsafe {
			let this = Self {
				slot: desc.into_rc_slot(),
				resource_context: cmd.resource_context(),
				_phantom: PhantomData,
				_phantom2: PhantomData,
			};
			this.transition_inner(f(this.inner_slot().access_lock.try_lock()?), A::IMAGE_ACCESS)?;
			Ok(this)
		}
	}

	/// Verify that this buffer has all the usages given by param.
	#[inline]
	pub fn has_required_usage(&self, required: BindlessImageUsage) -> Result<(), AccessError> {
		unsafe {
			let slot = self.inner_slot();
			if !slot.usage.contains(required) {
				Err(AccessError::MissingImageUsage {
					name: slot.debug_name().to_string(),
					usage: slot.usage,
					missing_usage: required,
				})
			} else {
				Ok(())
			}
		}
	}

	/// Transition this Image from one [`ImageAccessType`] to another and inserts appropriate barriers.
	pub fn transition<B: ImageAccessType>(self) -> MutImageAccess<'a, P, T, B> {
		self.try_transition::<B>().unwrap()
	}

	/// Transition this Image from one [`ImageAccessType`] to another and inserts appropriate barriers.
	pub fn try_transition<B: ImageAccessType>(self) -> Result<MutImageAccess<'a, P, T, B>, AccessError> {
		self.transition_inner(A::IMAGE_ACCESS, B::IMAGE_ACCESS)?;
		Ok(MutImageAccess {
			slot: self.slot,
			resource_context: self.resource_context,
			_phantom: PhantomData,
			_phantom2: PhantomData,
		})
	}

	#[inline]
	fn transition_inner(&self, src: ImageAccess, dst: ImageAccess) -> Result<(), AccessError> {
		unsafe {
			self.has_required_usage(dst.required_image_usage())?;
			if src != dst {
				self.resource_context.transition_image(&self.inner_slot(), src, dst)
			}
			Ok(())
		}
	}

	#[inline]
	pub unsafe fn inner_slot(&self) -> &ImageSlot<P> {
		ImageTable::get_slot(&self.slot)
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
			// cannot fail
			self.transition_inner(A::IMAGE_ACCESS, ImageAccess::GeneralRead)
				.unwrap();
			self.inner_slot().access_lock.unlock_to_shared();
			RCDesc::new(self.slot)
		}
	}
}

// TODO soundness: general layout may create Mut and ReadOnly Desc of a single Image. Aliasing them is UB in vulkan.
impl<'a, P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType + ShaderReadable> MutImageAccess<'a, P, T, A> {
	pub fn to_transient_storage(&self) -> TransientDesc<Image<T>> {
		self.try_to_transient_storage().unwrap()
	}

	pub fn try_to_transient_storage(&self) -> Result<TransientDesc<Image<T>>, AccessError> {
		self.has_required_usage(BindlessImageUsage::STORAGE)?;
		// Safety: mutable resource is in a layout that implements ShaderReadable, so it is readable by a shader
		unsafe {
			Ok(TransientDesc::new(
				self.slot.id(),
				&self.resource_context.to_transient_access(),
			))
		}
	}
}

impl<'a, P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType + ShaderSampleable> MutImageAccess<'a, P, T, A> {
	pub fn to_transient_sampled(&self) -> TransientDesc<Image<T>> {
		self.try_to_transient_sampled().unwrap()
	}

	pub fn try_to_transient_sampled(&self) -> Result<TransientDesc<Image<T>>, AccessError> {
		self.has_required_usage(BindlessImageUsage::SAMPLED)?;
		// Safety: mutable resource is in a layout that implements ShaderReadable, so it is readable by a shader
		unsafe {
			Ok(TransientDesc::new(
				self.slot.id(),
				&self.resource_context.to_transient_access(),
			))
		}
	}
}

impl<'a, P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType + ShaderReadWriteable>
	MutImageAccess<'a, P, T, A>
{
	pub fn to_mut_transient(&self) -> TransientDesc<MutImage<T>> {
		self.try_to_mut_transient().unwrap()
	}

	pub fn try_to_mut_transient(&self) -> Result<TransientDesc<MutImage<T>>, AccessError> {
		self.has_required_usage(BindlessImageUsage::STORAGE)?;
		// Safety: mutable resource is in a layout that implements ShaderReadWriteable, so it is readable and writeable
		// by a shader
		unsafe {
			Ok(TransientDesc::new(
				self.slot.id(),
				&self.resource_context.to_transient_access(),
			))
		}
	}
}
