use std::marker::PhantomData;

mod private {
	pub trait SealedTrait {}
}

/// A DType or DescriptorTableType is a sealed trait that defines the kind of DescriptorTable some Key is assigned for. The following DescriptorTables exist:
/// * [Buffer]
/// * [StorageImageType]
/// * [SampledImageType]
/// * [SamplerType]
pub trait DescType: private::SealedTrait {
	/// Associated non-generic [`ResourceTableCpu`]
	type ResourceTable: ResourceTable;
}

pub trait ResourceTable: private::SealedTrait {
	const BINDING: u32;
}

// Buffer
pub struct Buffer<T: ?Sized> {
	_phantom: PhantomData<T>,
}

pub struct BufferTable;

impl<T: ?Sized> private::SealedTrait for Buffer<T> {}

impl private::SealedTrait for BufferTable {}

impl<T: ?Sized> DescType for Buffer<T> {
	type ResourceTable = BufferTable;
}

impl ResourceTable for BufferTable {
	const BINDING: u32 = 0;
}

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
