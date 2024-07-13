//! loosely based on gltf's Schema, which is unfortunately not exported

use std::borrow::Cow;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
		} else if !uri.contains(":") {
			urlencoding::decode(uri).map_or(None, |path| Some(Scheme::RelativeFile(path)))
		} else {
			None
		}
	}

	pub fn read(&self, base_path: &Path) -> io::Result<SchemeReader> {
		Ok(match self {
			Scheme::Slice(slice) => SchemeReader::Slice(*slice),
			Scheme::Base64(data) => {
				let reader =
					base64::read::DecoderReader::new(data.as_bytes(), &base64::engine::general_purpose::STANDARD);
				SchemeReader::Base64(reader)
			}
			Scheme::AbsoluteFile(path) => SchemeReader::File(File::open(path)?),
			Scheme::RelativeFile(path) => {
				let absolute = PathBuf::from(base_path).join(path.as_ref());
				SchemeReader::File(File::open(absolute)?)
			}
		})
	}
}

pub enum SchemeReader<'a> {
	Slice(&'a [u8]),
	Base64(base64::read::DecoderReader<'static, base64::engine::GeneralPurpose, &'a [u8]>),
	File(File),
}

impl<'a> Read for SchemeReader<'a> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		match self {
			SchemeReader::Slice(slice) => slice.read(buf),
			SchemeReader::Base64(read) => read.read(buf),
			SchemeReader::File(read) => read.read(buf),
		}
	}
}
