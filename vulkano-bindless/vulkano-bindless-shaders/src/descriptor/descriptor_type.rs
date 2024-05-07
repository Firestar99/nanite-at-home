use crate::descriptor::descriptors::Descriptors;

pub(crate) mod private {
	pub trait SealedTrait {}
}

/// A DType or DescriptorTableType is a sealed trait that defines the kind of DescriptorTable some Key is assigned for. The following DescriptorTables exist:
/// * [`crate::descriptor::buffer::Buffer`]
/// * [StorageImageType]
/// * [SampledImageType]
/// * [SamplerType]
pub trait DescType: private::SealedTrait + 'static {
	type AccessType<'a>;

	fn access<'a>(descriptors: &'a Descriptors<'a>, id: u32) -> Self::AccessType<'a>;
}
