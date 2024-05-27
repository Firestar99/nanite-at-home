pub(crate) mod private {
	pub trait SealedTrait {}
}

/// A DescType is a sealed trait that defines the kinds of Descriptors that exist. The following descriptors exist:
/// * [`crate::descriptor::buffer::Buffer`]
/// * [`crate::descriptor::image::Image`]
/// * [`crate::descriptor::sampler::Sampler`]
pub trait DescType: private::SealedTrait + Send + Sync + 'static {
	type AccessType<'a>;

	const DESC_ENUM: DescEnum;
}

/// An enum of the kind of descriptor. Get it for any generic descriptor via [`DescType::DESC_ENUM`].
#[derive(Copy, Clone, Debug)]
pub enum DescEnum {
	Buffer,
	Image,
	Sampler,
}
