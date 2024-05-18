use bytemuck::AnyBitPattern;

pub trait ParamConstant: AnyBitPattern + Send + Sync {
	// type Static: ParamConstant<'static> + AnyBitPattern;
	//
	// /// Unsafely transmute TransientDesc lifetime to static
	// ///
	// /// # Safety
	// /// Must only be used just before writing push constants
	// unsafe fn to_static(&self) -> Self::Static;
}
