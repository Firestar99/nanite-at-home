pub(crate) mod private {
	pub trait SealedTrait {}
}

/// A DType or DescriptorTableType is a sealed trait that defines the kind of DescriptorTable some Key is assigned for. The following DescriptorTables exist:
/// * [`crate::descriptor::buffer::Buffer`]
/// * [StorageImageType]
/// * [SampledImageType]
/// * [SamplerType]
pub trait DescType: private::SealedTrait + Send + Sync + 'static {
	type AccessType<'a>;
}
