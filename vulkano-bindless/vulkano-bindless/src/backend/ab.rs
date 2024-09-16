use std::ops::{Index, IndexMut, Not};

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AB {
	A,
	B,
}

impl AB {
	pub fn to_u32(&self) -> u32 {
		*self as u32
	}

	pub fn from_u32(value: u32) -> Option<Self> {
		match value {
			0 => Some(AB::A),
			1 => Some(AB::B),
			_ => None,
		}
	}
}

impl Not for AB {
	type Output = AB;

	#[inline]
	fn not(self) -> Self::Output {
		match self {
			AB::A => AB::B,
			AB::B => AB::A,
		}
	}
}

/// An `[T; 2]` that can be indexed by [`AB`].
pub struct ABArray<T>([T; 2]);

impl<T> ABArray<T> {
	#[inline]
	pub fn new(f: impl FnMut() -> T) -> Self {
		Self([f(), f()])
	}
}

impl<T> Index<AB> for ABArray<T> {
	type Output = T;

	#[inline]
	fn index(&self, index: AB) -> &Self::Output {
		self.0.index(index as usize)
	}
}

impl<T> IndexMut<AB> for ABArray<T> {
	#[inline]
	fn index_mut(&mut self, index: AB) -> &mut Self::Output {
		self.0.index_mut(index as usize)
	}
}
