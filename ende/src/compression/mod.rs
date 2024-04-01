mod stream;

use std::io;
use std::io::{Read, Write};
use std::str::FromStr;
use parse_display::{Display, FromStr};
use thiserror::Error;
use crate::{Encoder, EncodingResult, Finish};

pub use stream::*;

/// Function for convenience.<br>
/// It calls [`Encoder::add_compression`] on the encoder with the given compression parameter,
/// calls the closure with the transformed encoder, then finalizes the compressor before returning
pub fn encode_with_compression<T, F>(
	encoder: &mut Encoder<T>,
	compression: Option<Compression>,
	f: F
) -> EncodingResult<()>
	where T: Write,
	      F: FnOnce(&mut Encoder<Compress<&mut T>>) -> EncodingResult<()>
{
	let mut encoder = encoder.add_compression(compression)?;
	let v = f(&mut encoder);
	encoder.finish()?.0.finish()?;
	v
}

/// Function for convenience.<br>
/// It calls [`Encoder::add_decompression`] on the decoder with the given compression parameter,
/// calls the closure with the transformed decoder, then finalizes the decompressor before returning
pub fn decode_with_compression<T, F, V>(
	decoder: &mut Encoder<T>,
	compression: Option<Compression>,
	f: F
) -> EncodingResult<V>
	where T: Read,
	      F: FnOnce(&mut Encoder<Decompress<&mut T>>) -> EncodingResult<V>,
	      V: crate::Decode
{
	let mut decoder = decoder.add_decompression(compression)?;
	let v = f(&mut decoder);
	decoder.finish()?.0.finish()?;
	v
}

/// Contains compression parameters known at a higher level than
/// the encoding/decoding step. Currently only consists of a [`Compression`] parameter,
/// but may be expanded in the future to accommodate for custom dictionaries.
#[derive(Clone, Eq, PartialEq, Debug, Display)]
#[display("compression = ({compression})")]
pub struct CompressionState {
	/// The compression parameter. This will be used to infer the compression mode when
	/// it is not known.
	pub compression: Compression
}

impl CompressionState {
	/// Constructs a new compression state, with the compression parameter set to None
	pub const fn new() -> Self {
		Self {
			compression: Compression::None,
		}
	}
}

/// ZStandard compression level
#[derive(
	Copy,
	Clone,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	Hash,
	Debug,
	Display,
	FromStr,
	ende_derive::Encode,
	ende_derive::Decode,
)]
#[ende(variant: fixed, 8)]
pub enum ZStdLevel {
	#[display("1")]
	L1 = 1,
	#[display("2")]
	L2 = 2,
	#[display("3")]
	L3 = 3,
	#[display("4")]
	L4 = 4,
	#[display("5")]
	L5 = 5,
	#[display("6")]
	L6 = 6,
	#[display("7")]
	L7 = 7,
	#[display("8")]
	L8 = 8,
	#[display("9")]
	L9 = 9,
	#[display("10")]
	L10 = 10,
	#[display("11")]
	L11 = 11,
	#[display("12")]
	L12 = 12,
	#[display("13")]
	L13 = 13,
	#[display("14")]
	L14 = 14,
	#[display("15")]
	L15 = 15,
	#[display("16")]
	L16 = 16,
	#[display("17")]
	L17 = 17,
	#[display("18")]
	L18 = 18,
	#[display("19")]
	L19 = 19,
	#[display("20")]
	L20 = 20,
	#[display("21")]
	L21 = 21,
	#[display("22")]
	L22 = 22,
}

/// ZLib compression level
#[derive(
	Copy,
	Clone,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	Hash,
	Debug,
	Display,
	FromStr,
	ende_derive::Encode,
	ende_derive::Decode,
)]
#[ende(variant: fixed, 8)]
pub enum ZLibLevel {
	#[display("0")]
	L0 = 0,
	#[display("1")]
	L1 = 1,
	#[display("2")]
	L2 = 2,
	#[display("3")]
	L3 = 3,
	#[display("4")]
	L4 = 4,
	#[display("5")]
	L5 = 5,
	#[display("6")]
	L6 = 6,
	#[display("7")]
	L7 = 7,
	#[display("8")]
	L8 = 8,
	#[display("9")]
	L9 = 9,
}

/// Deflate compression level
#[derive(
	Copy,
	Clone,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	Hash,
	Debug,
	Display,
	FromStr,
	ende_derive::Encode,
	ende_derive::Decode,
)]
#[ende(variant: fixed, 8)]
pub enum DeflateLevel {
	#[display("0")]
	L0 = 0,
	#[display("1")]
	L1 = 1,
	#[display("2")]
	L2 = 2,
	#[display("3")]
	L3 = 3,
	#[display("4")]
	L4 = 4,
	#[display("5")]
	L5 = 5,
	#[display("6")]
	L6 = 6,
	#[display("7")]
	L7 = 7,
	#[display("8")]
	L8 = 8,
	#[display("9")]
	L9 = 9,
}

/// GZip compression level
#[derive(
	Copy,
	Clone,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	Hash,
	Debug,
	Display,
	FromStr,
	ende_derive::Encode,
	ende_derive::Decode,
)]
#[ende(variant: fixed, 8)]
pub enum GZipLevel {
	#[display("1")]
	L1 = 1,
	#[display("2")]
	L2 = 2,
	#[display("3")]
	L3 = 3,
	#[display("4")]
	L4 = 4,
	#[display("5")]
	L5 = 5,
	#[display("6")]
	L6 = 6,
	#[display("7")]
	L7 = 7,
	#[display("8")]
	L8 = 8,
	#[display("9")]
	L9 = 9,
}

/// Compression algorithm and level, or None to indicate absence of compression.
/// Can be used to wrap a type implementing Write/Read in order to provide Compression/Decompression
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display, ende_derive::Encode, ende_derive::Decode)]
#[ende(variant: fixed, 8)]
pub enum Compression {
	#[display("no compression")]
	None,
	#[display("level {0} ZStd compression")]
	ZStd(ZStdLevel),
	#[display("level {0} ZLib compression")]
	ZLib(ZLibLevel),
	#[display("level {0} Deflate compression")]
	Deflate(DeflateLevel),
	#[display("level {0} GZip compression")]
	GZip(GZipLevel),
}

impl FromStr for Compression {
	type Err = &'static str;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		const USAGE: &str = r#"Invalid compression format. Usage: "{format}/{level}""#;

		if s == "None" {
			return Ok(Compression::None);
		}

		let (format, level) = s.split_once("/").ok_or(USAGE)?;

		Ok(match format {
			"ZStd" => Compression::ZStd(ZStdLevel::from_str(level).map_err(|_| "Out of range 1-22")?),
			"ZLib" => Compression::ZLib(ZLibLevel::from_str(level).map_err(|_| "Out of range 0-9")?),
			"Deflate" => Compression::Deflate(DeflateLevel::from_str(level).map_err(|_| "Out of range 0-9")?),
			"GZip" => Compression::GZip(GZipLevel::from_str(level).map_err(|_| "Out of range 1-9")?),
			_ => return Err(r#"Allowed compression formats are: ZStd, ZLib, Deflate, GZip"#)
		})
	}
}

impl Compression {
	/// Returns true if the `self` is None
	pub fn is_none(&self) -> bool {
		match self {
			Compression::None => true,
			_ => false
		}
	}

	/// Returns true if the `self` is ZStd
	pub fn is_zstd(&self) -> bool {
		match self {
			Compression::ZStd(..) => true,
			_ => false
		}
	}

	/// Returns true if the `self` is ZLib
	pub fn is_zlib(&self) -> bool {
		match self {
			Compression::ZLib(..) => true,
			_ => false
		}
	}

	/// Returns true if the `self` is Deflate
	pub fn is_deflate(&self) -> bool {
		match self {
			Compression::Deflate(..) => true,
			_ => false
		}
	}

	/// Returns true if the `self` is GZip
	pub fn is_gzip(&self) -> bool {
		match self {
			Compression::GZip(..) => true,
			_ => false
		}
	}
}

/// A generic error for anything that might go wrong during Compression/Decompression.<br>
/// FIXME This is still subject to change
#[derive(Debug, Error)]
pub enum CompressionError {
	/// Generic IO Error
	#[error("IO Error occurred: {0}")]
	IOError(
		#[source]
		#[from]
		io::Error
	)
}