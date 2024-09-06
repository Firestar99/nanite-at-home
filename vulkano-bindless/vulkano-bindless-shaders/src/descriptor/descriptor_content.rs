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
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum DescContentType {
	Buffer,
	Image,
	Sampler,
}
