use glam::{IVec2, IVec3, UVec2, UVec3};

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[must_use]
pub struct Extent {
	pub width: u32,
	pub height: u32,
	pub depth: u32,
}

impl From<[u32; 3]> for Extent {
	fn from(value: [u32; 3]) -> Self {
		Extent {
			width: value[0],
			height: value[1],
			depth: value[2],
		}
	}
}

impl From<[u32; 2]> for Extent {
	fn from(value: [u32; 2]) -> Self {
		Extent {
			width: value[0],
			height: value[1],
			depth: 1,
		}
	}
}

impl From<[u32; 1]> for Extent {
	fn from(value: [u32; 1]) -> Self {
		Extent {
			width: value[0],
			height: 1,
			depth: 1,
		}
	}
}

impl From<UVec3> for Extent {
	fn from(value: UVec3) -> Self {
		Extent {
			width: value.x,
			height: value.y,
			depth: value.z,
		}
	}
}

impl From<UVec2> for Extent {
	fn from(value: UVec2) -> Self {
		Extent {
			width: value.x,
			height: value.y,
			depth: 1,
		}
	}
}

impl From<u32> for Extent {
	fn from(value: u32) -> Self {
		Extent {
			width: value,
			height: 1,
			depth: 1,
		}
	}
}

impl From<Extent> for [u32; 3] {
	fn from(value: Extent) -> Self {
		[value.width, value.height, value.depth]
	}
}

impl From<Extent> for [u32; 2] {
	fn from(value: Extent) -> Self {
		[value.width, value.height]
	}
}

impl From<Extent> for [u32; 1] {
	fn from(value: Extent) -> Self {
		[value.width]
	}
}

impl From<Extent> for UVec3 {
	fn from(value: Extent) -> Self {
		UVec3::new(value.width, value.height, value.depth)
	}
}

impl From<Extent> for UVec2 {
	fn from(value: Extent) -> Self {
		UVec2::new(value.width, value.height)
	}
}

impl From<Extent> for u32 {
	fn from(value: Extent) -> Self {
		value.width
	}
}

impl Default for Extent {
	fn default() -> Self {
		Extent {
			width: 1,
			height: 1,
			depth: 1,
		}
	}
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[must_use]
pub struct Offset {
	pub width: i32,
	pub height: i32,
	pub depth: i32,
}

impl From<[i32; 3]> for Offset {
	fn from(value: [i32; 3]) -> Self {
		Offset {
			width: value[0],
			height: value[1],
			depth: value[2],
		}
	}
}

impl From<[i32; 2]> for Offset {
	fn from(value: [i32; 2]) -> Self {
		Offset {
			width: value[0],
			height: value[1],
			depth: 1,
		}
	}
}

impl From<[i32; 1]> for Offset {
	fn from(value: [i32; 1]) -> Self {
		Offset {
			width: value[0],
			height: 1,
			depth: 1,
		}
	}
}

impl From<IVec3> for Offset {
	fn from(value: IVec3) -> Self {
		Offset {
			width: value.x,
			height: value.y,
			depth: value.z,
		}
	}
}

impl From<IVec2> for Offset {
	fn from(value: IVec2) -> Self {
		Offset {
			width: value.x,
			height: value.y,
			depth: 1,
		}
	}
}

impl From<i32> for Offset {
	fn from(value: i32) -> Self {
		Offset {
			width: value,
			height: 1,
			depth: 1,
		}
	}
}

impl Default for Offset {
	fn default() -> Self {
		Offset {
			width: 1,
			height: 1,
			depth: 1,
		}
	}
}

impl From<Offset> for [i32; 3] {
	fn from(value: Offset) -> Self {
		[value.width, value.height, value.depth]
	}
}

impl From<Offset> for [i32; 2] {
	fn from(value: Offset) -> Self {
		[value.width, value.height]
	}
}

impl From<Offset> for [i32; 1] {
	fn from(value: Offset) -> Self {
		[value.width]
	}
}

impl From<Offset> for IVec3 {
	fn from(value: Offset) -> Self {
		IVec3::new(value.width, value.height, value.depth)
	}
}

impl From<Offset> for IVec2 {
	fn from(value: Offset) -> Self {
		IVec2::new(value.width, value.height)
	}
}

impl From<Offset> for i32 {
	fn from(value: Offset) -> Self {
		value.width
	}
}
