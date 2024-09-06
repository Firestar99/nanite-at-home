use num_derive::{FromPrimitive, ToPrimitive};
use static_assertions::const_assert;

pub(crate) mod private {
	pub trait SealedTrait {}
}

/// A DescType is a sealed trait that defines the kinds of Descriptors that exist. The following descriptors exist:
/// * [`crate::descriptor::buffer::Buffer`]
/// * [`crate::descriptor::image::Image`]
/// * [`crate::descriptor::sampler::Sampler`]
pub trait DescContent: private::SealedTrait + Sized + Send + Sync + 'static {
	type AccessType<'a>;

	const CONTENT_TYPE: DescContentType;
}

/// An enum of the kind of descriptor. Get it for any generic descriptor via [`DescContent::CONTENT_TYPE`].
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, FromPrimitive, ToPrimitive)]
pub enum DescContentType {
	Buffer,
	Image,
	Sampler,
}
// Insert amount of enum values here!           V
const_assert!((1 << super::id::ID_TYPE_BITS) >= 3);
