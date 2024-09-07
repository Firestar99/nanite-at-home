//! loosely based on gltf's Schema, which is unfortunately not exported

use base64::Engine;
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Eq, Hash, PartialEq)]
pub enum Scheme<'a> {
	/// simple slice
	Slice(&'a [u8]),
	/// `data:[<media type>];base64,<data>`.
	Base64(&'a str),
	/// `file:[//]<absolute file path>`
	AbsoluteFile(&'a str),
	/// `../foo`
	RelativeFile(Cow<'a, str>),
}

impl<'a> Scheme<'a> {
	pub fn from_slice(slice: &'a [u8]) -> Self {
		Self::Slice(slice)
	}

	pub fn parse(uri: &'a str) -> Option<Self> {
		if let Some(path) = uri.strip_prefix("file://") {
			Some(Scheme::AbsoluteFile(path))
		} else if let Some(path) = uri.strip_prefix("file:") {
			Some(Scheme::AbsoluteFile(path))
		} else if let Some(rest) = uri.strip_prefix("data:") {
			let mut it = rest.split(";base64,");
			match (it.next(), it.next()) {
				(_, Some(match1)) => Some(Scheme::Base64(match1)),
				(Some(match0), _) => Some(Scheme::Base64(match0)),
				_ => None,
			}
		} else if !uri.contains(':') {
			urlencoding::decode(uri).map_or(None, |path| Some(Scheme::RelativeFile(path)))
		} else {
			None
		}
	}

	pub fn read(&self, base_path: &Path) -> io::Result<Cow<'a, [u8]>> {
		Ok(match self {
			Scheme::Slice(slice) => Cow::Borrowed(*slice),
			Scheme::Base64(data) => Cow::Owned(
				base64::prelude::BASE64_STANDARD
					.decode(data.as_bytes())
					.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
			),
			Scheme::AbsoluteFile(path) => Cow::Owned(std::fs::read(path)?),
			Scheme::RelativeFile(path) => {
				let absolute = PathBuf::from(base_path).join(path.as_ref());
				Cow::Owned(std::fs::read(absolute)?)
			}
		})
	}
}

impl<'a> Debug for Scheme<'a> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			Scheme::Slice(slice) => write!(f, "Scheme(slice len: {})", slice.len()),
			Scheme::Base64(slice) => write!(f, "Scheme(base64 len: ~{})", base64::decoded_len_estimate(slice.len())),
			Scheme::AbsoluteFile(path) => write!(f, "Scheme(file://{})", path),
			Scheme::RelativeFile(path) => write!(f, "Scheme(./{})", path),
		}
	}
}
