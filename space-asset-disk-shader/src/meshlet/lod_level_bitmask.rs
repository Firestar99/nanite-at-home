use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};
use rust_gpu_bindless_macros::BufferStructPlain;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, BufferStructPlain)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct LodLevelBitmask(pub u32);

impl LodLevelBitmask {
	pub const fn all() -> Self {
		Self(!0)
	}

	pub const fn empty() -> Self {
		Self(0)
	}

	pub const fn contains(&self, other: LodLevelBitmask) -> bool {
		(self.0 & other.0) == 0
	}
}

impl Not for LodLevelBitmask {
	type Output = Self;
	fn not(self) -> Self {
		Self(!self.0)
	}
}

impl BitOr for LodLevelBitmask {
	type Output = Self;
	fn bitor(self, rhs: Self) -> Self {
		Self(self.0 | rhs.0)
	}
}

impl BitOrAssign for LodLevelBitmask {
	fn bitor_assign(&mut self, rhs: Self) {
		self.0 |= rhs.0;
	}
}

impl BitAnd for LodLevelBitmask {
	type Output = Self;
	fn bitand(self, rhs: Self) -> Self {
		Self(self.0 & rhs.0)
	}
}

impl BitAndAssign for LodLevelBitmask {
	fn bitand_assign(&mut self, rhs: Self) {
		self.0 &= rhs.0;
	}
}

impl BitXor for LodLevelBitmask {
	type Output = Self;
	fn bitxor(self, rhs: Self) -> Self {
		Self(self.0 ^ rhs.0)
	}
}

impl BitXorAssign for LodLevelBitmask {
	fn bitxor_assign(&mut self, rhs: Self) {
		self.0 ^= rhs.0;
	}
}
