use std::marker::PhantomData;
use std::ops::Deref;

mod private {
	pub trait DTypeIsASealedTrait {}
}

/// A DType or DescriptorTableType is a sealed trait that defines the kind of DescriptorTable some Key is assigned for. The following DescriptorTables exist:
/// * [Buffer]
/// * [StorageImageType]
/// * [SampledImageType]
/// * [SamplerType]
pub trait DescType: private::DTypeIsASealedTrait {}

pub struct Buffer<T: ?Sized> {
	_phantom: PhantomData<T>,
}

impl<T: ?Sized> private::DTypeIsASealedTrait for Buffer<T> {}

impl<T: ?Sized> DescType for Buffer<T> {}

// macro_rules! decl_dtype {
// 	($name:ident) => {
// 		paste! {
// 			pub enum [<$name Type>] {}
// 			impl private::DTypeIsASealedTrait for [<$name Type>] {}
// 			impl DType for [<$name Type>] {}
//
// 			// pub type [<$name Key>] = SlotKey<[<$name Type>]>;
// 			// pub type [<Weak $name Key>] = WeakSlotKey<[<$name Type>]>;
// 		}
// 	};
// }

// decl_dtype!(Buffer);
// decl_dtype!(StorageImage);
// decl_dtype!(SampledImage);
// decl_dtype!(Sampler);
