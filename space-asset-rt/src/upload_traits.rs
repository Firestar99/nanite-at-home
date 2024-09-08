pub trait ToStrong {
	type StrongType;
	fn to_strong(&self) -> Self::StrongType;
}
