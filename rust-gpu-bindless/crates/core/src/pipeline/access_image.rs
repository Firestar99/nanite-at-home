use crate::backing::table::RcTableSlot;
use crate::descriptor::{
	BindlessImageUsage, DescTable, Extent, Format, ImageDescExt, ImageSlot, ImageTable, RCDesc, RCDescExt,
};
use crate::descriptor::{MutDesc, MutDescExt};
use crate::pipeline::access_error::AccessError;
use crate::pipeline::access_type::{
	ImageAccess, ImageAccessType, ShaderReadWriteable, ShaderReadable, ShaderSampleable,
};
use crate::pipeline::mut_or_shared::MutOrSharedImage;
use crate::pipeline::recording::{HasResourceContext, Recording};
use crate::platform::{BindlessPipelinePlatform, RecordingResourceContext};
use rust_gpu_bindless_shaders::descriptor::{Image, ImageType, MutImage, TransientDesc};
use std::future::Future;
use std::marker::PhantomData;

pub trait MutImageAccessExt<P: BindlessPipelinePlatform, T: ImageType>: MutDescExt<P, MutImage<T>> {
	/// Access this mutable image to use it for recording.
	fn access<'a, A: ImageAccessType>(self, cmd: &Recording<'a, P>)
	-> Result<MutImageAccess<'a, P, T, A>, AccessError>;

	/// Access this mutable image to use it for recording. Discards the contents of this image and acts as if it were
	/// uninitialized.
	fn access_dont_care<'a, A: ImageAccessType>(
		self,
		cmd: &Recording<'a, P>,
	) -> Result<MutImageAccess<'a, P, T, A>, AccessError>;
}

impl<P: BindlessPipelinePlatform, T: ImageType> MutImageAccessExt<P, T> for MutDesc<P, MutImage<T>> {
	fn access<'a, A: ImageAccessType>(
		self,
		cmd: &Recording<'a, P>,
	) -> Result<MutImageAccess<'a, P, T, A>, AccessError> {
		MutImageAccess::from(self, cmd)
	}

	fn access_dont_care<'a, A: ImageAccessType>(
		self,
		cmd: &Recording<'a, P>,
	) -> Result<MutImageAccess<'a, P, T, A>, AccessError> {
		MutImageAccess::from_dont_care(self, cmd)
	}
}

pub struct MutImageAccess<'a, P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType> {
	slot: RcTableSlot,
	resource_context: &'a P::RecordingResourceContext,
	_phantom: PhantomData<T>,
	_phantom2: PhantomData<A>,
}

impl<'a, P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType> MutImageAccess<'a, P, T, A> {
	pub fn from_dont_care(desc: MutDesc<P, MutImage<T>>, cmd: &Recording<'a, P>) -> Result<Self, AccessError> {
		Self::from_inner(desc, cmd, |_| ImageAccess::Undefined)
	}

	pub fn from(desc: MutDesc<P, MutImage<T>>, cmd: &Recording<'a, P>) -> Result<Self, AccessError> {
		Self::from_inner(desc, cmd, |x| x)
	}

	#[inline]
	fn from_inner(
		desc: MutDesc<P, MutImage<T>>,
		cmd: &Recording<'a, P>,
		f: impl FnOnce(ImageAccess) -> ImageAccess,
	) -> Result<Self, AccessError> {
		unsafe {
			let (slot, last) = desc.into_inner();
			cmd.resource_context().add_dependency(last);
			let this = Self {
				slot,
				resource_context: cmd.resource_context(),
				_phantom: PhantomData,
				_phantom2: PhantomData,
			};
			this.transition_inner(f(this.inner_slot().access_lock.try_lock()?), A::IMAGE_ACCESS)?;
			Ok(this)
		}
	}

	/// Transition this Image from one [`ImageAccessType`] to another and inserts appropriate barriers.
	pub fn transition<B: ImageAccessType>(self) -> Result<MutImageAccess<'a, P, T, B>, AccessError> {
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
				self.resource_context.transition_image(self.inner_slot(), src, dst)
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
			MutDesc::new(self.slot, self.resource_context.to_pending_execution())
		}
	}

	/// Turns this mutable access to a [`MutImage`] into a shared [`RCDesc`]
	pub fn into_shared(self) -> impl Future<Output = RCDesc<P, Image<T>>> + use<P, T, A> {
		unsafe {
			// cannot fail
			self.transition_inner(A::IMAGE_ACCESS, ImageAccess::GeneralRead)
				.unwrap();
			let pending_execution = self.resource_context.to_pending_execution();
			let slot = self.slot;
			async move {
				pending_execution.await;
				ImageTable::<P>::get_slot(&slot).access_lock.unlock_to_shared();
				RCDesc::new(slot)
			}
		}
	}
}

impl<P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType> ImageDescExt for MutImageAccess<'_, P, T, A> {
	fn extent(&self) -> Extent {
		unsafe { self.inner_slot().extent }
	}

	fn format(&self) -> Format {
		unsafe { self.inner_slot().format }
	}
}

// TODO soundness: general layout may create Mut and ReadOnly Desc of a single Image. Aliasing them is UB in vulkan.
impl<P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType + ShaderReadable> MutImageAccess<'_, P, T, A> {
	pub fn to_transient_storage(&self) -> Result<TransientDesc<'_, Image<T>>, AccessError> {
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

impl<P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType + ShaderSampleable> MutImageAccess<'_, P, T, A> {
	pub fn to_transient_sampled(&self) -> Result<TransientDesc<'_, Image<T>>, AccessError> {
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

impl<P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType + ShaderReadWriteable> MutImageAccess<'_, P, T, A> {
	pub fn to_mut_transient(&self) -> TransientDesc<'_, MutImage<T>> {
		self.try_to_mut_transient().unwrap()
	}

	pub fn try_to_mut_transient(&self) -> Result<TransientDesc<'_, MutImage<T>>, AccessError> {
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
