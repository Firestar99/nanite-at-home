pub(crate) mod private {
	pub trait SealedTrait {}
}

/// A DescType is a sealed trait that defines the kinds of Descriptors that exist. The following descriptors exist:
/// * [`crate::descriptor::buffer::Buffer`]
/// * [`crate::descriptor::image::Image`]
/// * [`crate::descriptor::sampler::Sampler`]
pub trait DescType: private::SealedTrait + Send + Sync + 'static {
	type AccessType<'a>;
}
