use std::io;
use std::io::{BufReader, Read, Write};
use parse_display::Display;
use thiserror::Error;
use crate::{BinStream, EncodingResult, Encode, Decode, Finish};

pub fn encode_with_compression<T, F>(
	encoder: &mut BinStream<T>,
	compression: Compression,
	f: F
) -> EncodingResult<()>
	where T: Write,
	      F: FnOnce(&mut BinStream<Compress<&mut T>>) -> EncodingResult<()>
{
	let mut encoder = encoder.add_compression(compression)?;
	let v = f(&mut encoder);
	encoder.finish()?.finish()?;
	v
}

pub fn decode_with_compression<T, F, V>(
	decoder: &mut BinStream<T>,
	compression: Compression,
	f: F
) -> EncodingResult<V>
	where T: Read,
	      F: FnOnce(&mut BinStream<Decompress<&mut T>>) -> EncodingResult<V>,
	      V: Decode
{
	let mut decoder = decoder.add_decompression(compression)?;
	let v = f(&mut decoder);
	decoder.finish()?.finish()?;
	v
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Display, Encode, Decode)]
#[repr(u8)]
#[ende(variant: 8)]
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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Display, Encode, Decode)]
#[repr(u8)]
#[ende(variant: 8)]
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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Display, Encode, Decode)]
#[repr(u8)]
#[ende(variant: 8)]
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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Display, Encode, Decode)]
#[repr(u8)]
#[ende(variant: 8)]
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

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display, Encode, Decode)]
#[repr(u8)]
#[ende(variant: 8)]
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

impl Compression {
	pub fn is_none(&self) -> bool {
		match self {
			Compression::None => true,
			_ => false
		}
	}

	pub fn is_zstd(&self) -> bool {
		match self {
			Compression::ZStd(..) => true,
			_ => false
		}
	}

	pub fn is_zlib(&self) -> bool {
		match self {
			Compression::ZLib(..) => true,
			_ => false
		}
	}

	pub fn is_deflate(&self) -> bool {
		match self {
			Compression::Deflate(..) => true,
			_ => false
		}
	}

	pub fn is_gzip(&self) -> bool {
		match self {
			Compression::GZip(..) => true,
			_ => false
		}
	}
}

impl Compression {
	pub fn compress<T: Write>(&self, input: T) -> Result<Compress<T>, CompressionError> {
		match self {
			Compression::None => {
				Ok(Compress(CompressInner::None(input)))
			}
			Compression::ZStd(level) => {
				Ok(Compress(CompressInner::ZStd(zstd::stream::write::Encoder::new(input, *level as _)?)))
			}
			Compression::ZLib(level) => {
				Ok(Compress(CompressInner::ZLib(flate2::write::ZlibEncoder::new(input, flate2::Compression::new(*level as _)))))
			}
			Compression::Deflate(level) => {
				Ok(Compress(CompressInner::Deflate(flate2::write::DeflateEncoder::new(input, flate2::Compression::new(*level as _)))))
			}
			Compression::GZip(level) => {
				Ok(Compress(CompressInner::GZip(flate2::write::GzEncoder::new(input, flate2::Compression::new(*level as _)))))
			}
		}
	}

	pub fn decompress<T: Read>(&self, input: T) -> Result<Decompress<T>, CompressionError> {
		match self {
			Compression::None => {
				Ok(Decompress(DecompressInner::None(input)))
			}
			Compression::ZStd(..) => {
				Ok(Decompress(DecompressInner::ZStd(zstd::stream::read::Decoder::new(input)?)))
			}
			Compression::ZLib(..) => {
				Ok(Decompress(DecompressInner::ZLib(flate2::read::ZlibDecoder::new(input))))
			}
			Compression::Deflate(..) => {
				Ok(Decompress(DecompressInner::Deflate(flate2::read::DeflateDecoder::new(input))))
			}
			Compression::GZip(..) => {
				Ok(Decompress(DecompressInner::GZip(flate2::read::GzDecoder::new(input))))
			}
		}
	}
}

enum CompressInner<T: Write> {
	None(T),
	ZStd(zstd::stream::write::Encoder<'static, T>),
	ZLib(flate2::write::ZlibEncoder<T>),
	Deflate(flate2::write::DeflateEncoder<T>),
	GZip(flate2::write::GzEncoder<T>),
}

impl<T: Write> Finish for CompressInner<T> {
	type Output = T;
	#[inline]
	fn finish(self) -> EncodingResult<Self::Output> {
		match self {
			CompressInner::None(x) => Ok(x),
			CompressInner::ZStd(x) => Ok(x.finish()?),
			CompressInner::ZLib(x) => Ok(x.finish()?),
			CompressInner::Deflate(x) => Ok(x.finish()?),
			CompressInner::GZip(x) => Ok(x.finish()?),
		}
	}
}

impl<T: Write> Write for CompressInner<T> {
	#[inline]
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		match self {
			CompressInner::None(x) => x.write(buf),
			CompressInner::ZStd(x) => x.write(buf),
			CompressInner::ZLib(x) => x.write(buf),
			CompressInner::Deflate(x) => x.write(buf),
			CompressInner::GZip(x) => x.write(buf),
		}
	}
	#[inline]
	fn flush(&mut self) -> io::Result<()> {
		match self {
			CompressInner::None(x) => x.flush(),
			CompressInner::ZStd(x) => x.flush(),
			CompressInner::ZLib(x) => x.flush(),
			CompressInner::Deflate(x) => x.flush(),
			CompressInner::GZip(x) => x.flush(),
		}
	}
}

#[repr(transparent)]
pub struct Compress<T: Write>(CompressInner<T>);

impl<T: Write> Finish for Compress<T> {
	type Output = T;
	#[inline]
	fn finish(self) -> EncodingResult<Self::Output> {
		self.0.finish()
	}
}

impl<T: Write> Write for Compress<T> {
	#[inline]
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		self.0.write(buf)
	}
	#[inline]
	fn flush(&mut self) -> io::Result<()> {
		self.0.flush()
	}
}

enum DecompressInner<T: Read> {
	None(T),
	ZStd(zstd::stream::read::Decoder<'static, BufReader<T>>),
	ZLib(flate2::read::ZlibDecoder<T>),
	Deflate(flate2::read::DeflateDecoder<T>),
	GZip(flate2::read::GzDecoder<T>),
}

impl<T: Read> Finish for DecompressInner<T> {
	type Output = T;
	#[inline]
	fn finish(self) -> EncodingResult<Self::Output> {
		match self {
			DecompressInner::None(x) => Ok(x),
			DecompressInner::ZStd(x) => Ok(x.finish().into_inner()),
			DecompressInner::ZLib(x) => Ok(x.into_inner()),
			DecompressInner::Deflate(x) => Ok(x.into_inner()),
			DecompressInner::GZip(x) => Ok(x.into_inner()),
		}
	}
}

impl<T: Read> Read for DecompressInner<T> {
	#[inline]
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		match self {
			DecompressInner::None(x) => x.read(buf),
			DecompressInner::ZStd(x) => x.read(buf),
			DecompressInner::ZLib(x) => x.read(buf),
			DecompressInner::Deflate(x) => x.read(buf),
			DecompressInner::GZip(x) => x.read(buf),
		}
	}
}

#[repr(transparent)]
pub struct Decompress<T: Read>(DecompressInner<T>);

impl<T: Read> Finish for Decompress<T> {
	type Output = T;
	#[inline]
	fn finish(self) -> EncodingResult<Self::Output> {
		self.0.finish()
	}
}

impl<T: Read> Read for Decompress<T> {
	#[inline]
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		self.0.read(buf)
	}
}

#[derive(Debug, Error)]
pub enum CompressionError {
	#[error("IO Error occurred: {0}")]
	IOError(
		#[source]
		#[from]
		io::Error
	)
}