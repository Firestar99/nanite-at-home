/// Metadata about an execution, like the current frame in flight, to be able to safely upgrade weak pointers.
/// Currently unused.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Metadata;
