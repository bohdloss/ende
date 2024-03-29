#[cfg(test)]
mod test;

#[cfg(feature = "encryption")]
pub mod encryption;
#[cfg(feature = "compression")]
pub mod compression;
#[cfg(feature = "serde")]
pub mod serde;

use std::collections::HashMap;
use std::ffi::{CStr, CString, FromVecWithNulError};
use std::hash::Hash;
use std::io;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::mem::replace;
use std::string::FromUtf8Error;
use array_init::array_init;
use thiserror::Error;

#[cfg(feature = "derive")]
pub use ende_derive::{Decode, Encode};
use parse_display::Display;

/// Encodes the given value by constructing an encoder on the fly and using it to wrap the writer,
/// with the given context.
pub fn encode_with<T: Write, V: Encode>(writer: T, context: Context, value: V) -> EncodingResult<()> {
	let mut stream = Encoder::new(writer, context);
	value.encode(&mut stream)
}

/// Decodes the given value by constructing an encoder on the fly and using it to wrap the writer,
/// with the given context.
pub fn decode_with<T: Read, V: Decode>(reader: T, context: Context) -> EncodingResult<V> {
	let mut stream = Encoder::new(reader, context);
	V::decode(&mut stream)
}

/// Controls the endianness of a numerical value. Endianness is just
/// the order in which the value's bytes are written.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display)]
#[repr(u8)]
pub enum Endianness {
	/// Least significant bytes first
	LittleEndian,
	/// Most significant bytes first
	BigEndian
}

/// Controls the encoding of a numerical value
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display)]
#[repr(u8)]
pub enum NumEncoding {
	/// Its bits are encoded as-is according to endianness
	Fixed,
	/// Its bits are encoded according to the LEB128 (Little Endian Base 128) standard
	/// if unsigned, or ULEB128 standard if signed
	Leb128
}

/// How many bits a size or enum variant will occupy in the binary format. If the value
/// contains more bits, they will be trimmed (lost), so change this value with care
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display)]
#[repr(u8)]
pub enum BitWidth {
	/// Max 8 bits per value
	Bit8,
	/// Max 16 bits per value
	Bit16,
	/// Max 32 bits per value
	Bit32,
	/// Max 64 bits per value
	Bit64,
	/// Max 128 bits per value
	Bit128
}

/// Controls the binary representation of numbers (different from sizes and enum variants).
/// Specifically, controls the [`Endianness`] and [`NumEncoding`].
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display)]
#[display("endianness = {endianness}, encoding = {num_encoding}")]
pub struct NumRepr {
	pub endianness: Endianness,
	pub num_encoding: NumEncoding,
}

impl NumRepr {
	/// Returns the default numerical representation: little endian with fixed encoding
	pub const fn new() -> Self {
		Self {
			endianness: Endianness::LittleEndian,
			num_encoding: NumEncoding::Fixed
		}
	}
}

impl Default for NumRepr {
	fn default() -> Self {
		Self::new()
	}
}

/// Controls the binary representation of sizes.
/// Specifically, controls the [`Endianness`], the [`NumEncoding`], the [`BitWidth`],
/// and the greatest encodable/decodable size before an error is thrown
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display)]
#[display("endianness = {endianness} , encoding = {num_encoding}, bit_width = {width}, max_size = {max_size}")]
pub struct SizeRepr {
	pub endianness: Endianness,
	pub num_encoding: NumEncoding,
	pub width: BitWidth,
	pub max_size: usize
}

impl SizeRepr {
	/// Returns the default size representation: little endian, fixed encoding, 64 bit width,
	/// and the max size set to `usize::MAX`
	pub const fn new() -> Self {
		Self {
			endianness: Endianness::LittleEndian,
			num_encoding: NumEncoding::Fixed,
			width: BitWidth::Bit64,
			max_size: usize::MAX,
		}
	}
}

impl Default for SizeRepr {
	fn default() -> Self {
		Self::new()
	}
}

/// Controls the binary representation of enum variants.
/// Specifically, controls the [`Endianness`], the [`NumEncoding`], and the [`BitWidth`].
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display)]
#[display("endianness = {endianness} , encoding = {num_encoding}, bit_width = {width}")]
pub struct VariantRepr {
	pub endianness: Endianness,
	pub num_encoding: NumEncoding,
	pub width: BitWidth
}

impl VariantRepr {
	/// Returns the default variant representation: little endian, fixed encoding and 32 bit width
	pub const fn new() -> Self {
		Self {
			endianness: Endianness::LittleEndian,
			num_encoding: NumEncoding::Fixed,
			width: BitWidth::Bit32
		}
	}
}

impl Default for VariantRepr {
	fn default() -> Self {
		Self::new()
	}
}

/// An aggregation of [`NumRepr`], [`SizeRepr`], [`VariantRepr`]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display)]
#[display("num_repr = ({num_repr}), size_repr = ({size_repr}), variant_repr = ({variant_repr})")]
pub struct BinSettings {
	pub num_repr: NumRepr,
	pub size_repr: SizeRepr,
	pub variant_repr: VariantRepr,
}

impl BinSettings {
	/// Returns the default options containing the default for each representation.
	/// See: [`NumRepr::new`], [`SizeRepr::new`], [`VariantRepr::new`]
	pub const fn new() -> Self {
		Self {
			num_repr: NumRepr::new(),
			size_repr: SizeRepr::new(),
			variant_repr: VariantRepr::new(),
		}
	}
}

impl Default for BinSettings {
	fn default() -> Self {
		Self::new()
	}
}

/// The state of the encoder, including its options, `flatten` state variable,
/// a crypto state if the `encryption` feature is enabled
#[derive(Clone, Debug)]
pub struct Context<'a> {
	/// The lifetime `'a` is used by the crypto state, only present when the `encryption` feature
	/// is enabled.<br> In order to ensure compatibility, we must use
	/// the lifetime even when the feature is disabled.
	pub phantom_lifetime: PhantomData<&'a ()>,
	/// The actual settings, which determine the numerical representations and the string
	/// representations. <br>Implementations of [`Encode`] and [`Decode`] are required to
	/// preserve the state of the settings, even though they are allowed to temporarily modify it.<br>
	/// In case of an error occurring, no guarantee is made about the state of the settings:
	/// for this reason it's good practice to store a copy of the settings somewhere.
	pub settings: BinSettings,
	/// The flatten state variable. When present, for `Option` it indicates in Encode mode
	/// not to write whether the optional is present, and in Decode mode that it is present (without
	/// checking), for `Vec`, `HashMap` and other data structures with a length it indicates in
	/// Encode mode not to write said length, and in Decode mode the length itself.
	pub flatten: Option<usize>,
	/// Keeps track of the lengths of maps and vectors in recursive serde `deserialize` calls
	#[cfg(feature = "serde")]
	pub len_stack: smallvec::SmallVec<usize, 8>,
	/// The cryptographic state. See [`encryption::CryptoState`]
	#[cfg(feature = "encryption")]
	pub crypto: encryption::CryptoState<'a>,
	#[cfg(feature = "compression")]
	pub compression: compression::CompressionState,
}

impl<'a> Context<'a> {
	/// Constructs the default encoder state. Options will be set to default, flatten to None,
	/// and crypto state to default
	pub fn new() -> Self {
		Self {
			phantom_lifetime: PhantomData,
			settings: BinSettings::new(),
			flatten: None,
			#[cfg(feature = "serde")]
			len_stack: smallvec::SmallVec::new(),
			#[cfg(feature = "encryption")]
			crypto: encryption::CryptoState::new(),
			#[cfg(feature = "compression")]
			compression: compression::CompressionState::new(),
		}
	}

	/// Similar to clone, but hints that the keys being stored should not be cloned to
	/// a new memory location, but simply borrowed.
	/// Only really useful if the `encryption` feature is enabled.
	pub fn borrow_clone(&self) -> Context {
		Context {
			phantom_lifetime: PhantomData,
			settings: self.settings,
			flatten: self.flatten,
			#[cfg(feature = "serde")]
			len_stack: smallvec::SmallVec::new(),
			#[cfg(feature = "encryption")]
			crypto: self.crypto.borrow_clone(),
			#[cfg(feature = "compression")]
			compression: self.compression,
		}
	}

	/// Uses the given options and, if the `encryption` feature is enabled, the crypto state will
	/// be initialized to default
	pub fn with_options(options: BinSettings) -> Self {
		Self {
			phantom_lifetime: PhantomData,
			settings: options,
			flatten: None,
			#[cfg(feature = "serde")]
			len_stack: smallvec::SmallVec::new(),
			#[cfg(feature = "encryption")]
			crypto: encryption::CryptoState::new(),
			#[cfg(feature = "compression")]
			compression: compression::CompressionState::new(),
		}
	}

	/// Uses the given options and crypto state
	#[cfg(feature = "encryption")]
	pub fn with_crypto_state(options: BinSettings, crypto: encryption::CryptoState<'a>) -> Self {
		Self {
			phantom_lifetime: PhantomData,
			settings: options,
			flatten: None,
			#[cfg(feature = "serde")]
			len_stack: smallvec::SmallVec::new(),
			crypto,
			#[cfg(feature = "compression")]
			compression: compression::CompressionState::new(),
		}
	}

	/// Resets the state to its defaults, then overwrites the options with the given options
	pub fn reset(&mut self, options: BinSettings) {
		self.settings = options;
		self.flatten = None;
		#[cfg(feature = "serde")]
		{
			self.len_stack.clear();
		}
		#[cfg(feature = "compression")]
		{
			self.compression.compression = compression::Compression::None;
		}
		#[cfg(feature = "encryption")]
		{
			self.crypto.asymm.reset_public();
			self.crypto.asymm.reset_private();
			self.crypto.symm.reset_key();
			self.crypto.symm.reset_iv();
		}
	}

	/// Returns the state of the `flatten` variable, consuming it
	pub fn flatten(&mut self) -> Option<usize> {
		let old = self.flatten;
		self.flatten = None;
		old
	}
}

/// A helper trait used to indicate that a type (usually a stream) can unwrap to its inner type
/// and perform some form of cleanup. This trait is implemented for Encryptors and Compressors
/// for example to pad the inner stream to the next full block
pub trait Finish {
	type Output;
	fn finish(self) -> EncodingResult<Self::Output>;
}

/// The base type for encoding/decoding. References a stream, and a [`Context`].<br>
/// It's recommended to wrap the stream in a [`std::io::BufReader`] or [`std::io::BufWriter`],
/// because many small write and read calls will be made
pub struct Encoder<'a, T>{
	/// The underlying stream
	pub stream: T,
	/// The state
	pub ctxt: Context<'a>,
}

impl<'a, T> Encoder<'a, T> {
	/// Wraps the given stream and state.
	pub fn new(stream: T, ctxt: Context<'a>) -> Self {
		Self {
			stream,
			ctxt
		}
	}

	/// Replaces the underlying stream with the new one, returning the previous value
	pub fn swap_stream(&mut self, new: T) -> T {
		replace(&mut self.stream, new)
	}
}

impl<T: Write> Encoder<'_, T> {
	/// Method for convenience.<br>
	/// Encodes a value using `self` as the encoder.<br>
	/// This method is not magic - it is literally defined as `value.encode(self)`
	pub fn encode_value<V: Encode>(&mut self, value: V) -> EncodingResult<()> {
		value.encode(self)
	}
}

impl<T: Read> Encoder<'_, T> {
	/// Method for convenience.<br>
	/// Decodes a value using `self` as the decoder.<br>
	/// This method is not magic - it is literally defined as `V::decode(self)`
	pub fn decode_value<V: Decode>(&mut self) -> EncodingResult<V> {
		V::decode(self)
	}
}

impl<T: Write> Encoder<'_, T> {
	/// Returns a BinStream with the same context,
	/// but wraps the underlying stream in an [`encryption::Encrypt`].
	/// When either the key or the iv are None, this function will try to fetch them
	/// from the crypto state
	#[cfg(feature = "encryption")]
	pub fn add_encryption(&mut self, encryption: Option<encryption::SymmEncryption>, key: Option<&[u8]>, iv: Option<&[u8]>) -> EncodingResult<Encoder<encryption::Encrypt<&mut T>>> {
		let encryption = encryption.unwrap_or(self.ctxt.crypto.symm.encryption);
		let key = key.or(self.ctxt.crypto.symm.get_key());
		let iv = iv.or(self.ctxt.crypto.symm.get_iv());
		Ok(Encoder::new(encryption.encrypt(&mut self.stream, key, iv)?, self.ctxt.borrow_clone()))
	}

	/// Returns an Encoder with the same context,
	/// but wraps the underlying stream in an [`compression::Compress`]
	#[cfg(feature = "compression")]
	pub fn add_compression(&mut self, compression: Option<compression::Compression>) -> EncodingResult<Encoder<compression::Compress<&mut T>>> {
		let compression = compression.unwrap_or(self.ctxt.compression.compression);
		return Ok(Encoder::new(compression.compress(&mut self.stream)?, self.ctxt.borrow_clone()));
	}
}

impl<T: Read> Encoder<'_, T> {
	/// Returns an Encoder with the same context,
	/// but wraps the underlying stream in an [`encryption::Decrypt`]
	/// When either the key or the iv are None, this function will try to fetch them
	/// from the crypto state
	#[cfg(feature = "encryption")]
	pub fn add_decryption(&mut self, encryption: Option<encryption::SymmEncryption>, key: Option<&[u8]>, iv: Option<&[u8]>) -> EncodingResult<Encoder<encryption::Decrypt<&mut T>>> {
		let encryption = encryption.unwrap_or(self.ctxt.crypto.symm.encryption);
		let key = key.or(self.ctxt.crypto.symm.get_key());
		let iv = iv.or(self.ctxt.crypto.symm.get_iv());
		Ok(Encoder::new(encryption.decrypt(&mut self.stream, key, iv)?, self.ctxt.borrow_clone()))
	}

	/// Returns an Encoder with the same context,
	/// but wraps the underlying stream in an [`compression::Decompress`]
	#[cfg(feature = "compression")]
	pub fn add_decompression(&mut self, compression: Option<compression::Compression>) -> EncodingResult<Encoder<compression::Decompress<&mut T>>> {
		let compression = compression.unwrap_or(self.ctxt.compression.compression);
		return Ok(Encoder::new(compression.decompress(&mut self.stream)?, self.ctxt.borrow_clone()));
	}
}

impl<'a, T> Finish for Encoder<'a, T> {
	type Output = (T, Context<'a>);
	fn finish(self) -> EncodingResult<Self::Output> {
		Ok((self.stream, self.ctxt))
	}
}

macro_rules! make_unsigned_write_fn {
    ($write_internal:ident => $write_size:ident => $write_variant:ident => $write:ident => $ty:ty) => {
	    #[doc = "Encodes a `"]
	    #[doc = stringify!($ty)]
	    #[doc = "` to the underlying stream, according to the endianness and numerical encoding in the encoder's state"]
	    pub fn $write(&mut self, value: $ty) -> EncodingResult<()> {
		    self.$write_internal(value, self.ctxt.settings.num_repr.num_encoding, self.ctxt.settings.num_repr.endianness)
	    }
	    
	    fn $write_size(&mut self, value: $ty) -> EncodingResult<()> {
		    self.$write_internal(value, self.ctxt.settings.size_repr.num_encoding, self.ctxt.settings.size_repr.endianness)
	    }
	    
	    fn $write_variant(&mut self, value: $ty) -> EncodingResult<()> {
		    self.$write_internal(value, self.ctxt.settings.variant_repr.num_encoding, self.ctxt.settings.variant_repr.endianness)
	    }

        fn $write_internal(&mut self, value: $ty, num_encoding: NumEncoding, endianness: Endianness) -> EncodingResult<()> {
	        match num_encoding {
		        NumEncoding::Fixed => {
			        let bytes: [u8; std::mem::size_of::<$ty>()] = match endianness {
			            Endianness::BigEndian => value.to_be_bytes(),
			            Endianness::LittleEndian => value.to_le_bytes()
		            };
		            self.stream.write_all(&bytes)?;
		        },
		        NumEncoding::Leb128 => {
			        let mut shifted = value;
			        let mut byte = [u8::MAX; 1];
			        let mut more = true;
			        while more {
				        byte[0] = shifted as u8 & 0b01111111;
				        shifted >>= 7;
				        
				        // Is the next shifted value worth writing?
				        if shifted != 0 {
					        byte[0] |= 0b10000000;
				        } else {
					        more = false;
				        }
				        self.stream.write_all(&byte)?;
					}
		        }
	        }
            Ok(())
        }
    };
}

macro_rules! make_signed_write_fn {
    ($write_internal:ident => $write_size:ident => $write_variant:ident => $write:ident => $ty:ty) => {
	    #[doc = "Encodes a `"]
	    #[doc = stringify!($ty)]
	    #[doc = "` to the underlying stream, according to the endianness and numerical encoding in the encoder's state"]
	    pub fn $write(&mut self, value: $ty) -> EncodingResult<()> {
		    self.$write_internal(value, self.ctxt.settings.num_repr.num_encoding, self.ctxt.settings.num_repr.endianness)
	    }
	    
	    fn $write_size(&mut self, value: $ty) -> EncodingResult<()> {
		    self.$write_internal(value, self.ctxt.settings.size_repr.num_encoding, self.ctxt.settings.size_repr.endianness)
	    }
	    
	    fn $write_variant(&mut self, value: $ty) -> EncodingResult<()> {
		    self.$write_internal(value, self.ctxt.settings.variant_repr.num_encoding, self.ctxt.settings.variant_repr.endianness)
	    }

        fn $write_internal(&mut self, value: $ty, num_encoding: NumEncoding, endianness: Endianness) -> EncodingResult<()> {
	        match num_encoding {
		        NumEncoding::Fixed => {
			        let bytes: [u8; std::mem::size_of::<$ty>()] = match endianness {
			            Endianness::BigEndian => value.to_be_bytes(),
			            Endianness::LittleEndian => value.to_le_bytes()
		            };
		            self.stream.write_all(&bytes)?;
		        },
		        NumEncoding::Leb128 => {
			        let mut shifted = value;
			        let mut byte = [0u8; 1];
			        let mut more = true;
			        while more {
				        byte[0] = shifted as u8 & 0b0111_1111;
				        shifted >>= 7;
				        
				        // Is the next shifted value worth writing?
				        let neg = (byte[0] & 0b0100_0000) != 0;
				        if (neg && shifted != -1) || (!neg && shifted != 0) {
					        byte[0] |= 0b1000_0000;
				        } else {
					        more = false;
				        }
				        self.stream.write_all(&byte)?;
					}
		        }
	        }
            Ok(())
        }
    };
}

macro_rules! make_unsigned_read_fn {
    ($read_internal:ident => $read_size:ident => $read_variant:ident => $read:ident => $ty:ty) => {
	    #[doc = "Decodes a `"]
	    #[doc = stringify!($ty)]
	    #[doc = "` from the underlying stream, according to the endianness and numerical encoding in the encoder's state"]
	    pub fn $read(&mut self) -> EncodingResult<$ty> {
		    self.$read_internal(self.ctxt.settings.num_repr.num_encoding, self.ctxt.settings.num_repr.endianness)
	    }
	    
	    fn $read_size(&mut self) -> EncodingResult<$ty> {
		    self.$read_internal(self.ctxt.settings.size_repr.num_encoding, self.ctxt.settings.size_repr.endianness)
	    }
	    
	    fn $read_variant(&mut self) -> EncodingResult<$ty> {
		    self.$read_internal(self.ctxt.settings.variant_repr.num_encoding, self.ctxt.settings.variant_repr.endianness)
	    }
	    
        fn $read_internal(&mut self, num_encoding: NumEncoding, endianness: Endianness) -> EncodingResult<$ty> {
	        Ok(match num_encoding {
		        NumEncoding::Fixed => {
			        let mut bytes: [u8; std::mem::size_of::<$ty>()] = [0u8; std::mem::size_of::<$ty>()];
		            self.stream.read_exact(&mut bytes)?;

		            match endianness {
			            Endianness::BigEndian => <$ty>::from_be_bytes(bytes),
			            Endianness::LittleEndian => <$ty>::from_le_bytes(bytes)
		            }
		        }
		        NumEncoding::Leb128 => {
			        let mut result: $ty = 0;
			        let mut byte = [0u8; 1];
			        let mut shift: u8 = 0;
			        loop {
				        if shift >= <$ty>::BITS as u8 {
					        return Err(EncodingError::VarIntError);
				        }
				        
			            self.stream.read_exact(&mut byte)?;
				        result |= (byte[0] & 0b0111_1111) as $ty << shift;
				        shift += 7;
				        
				        if (byte[0] & 0b1000_0000) == 0 {
					        break;
				        }
					}
			        result
		        }
	        })
        }
    };
}

macro_rules! make_signed_read_fn {
    ($read_internal:ident => $read_size:ident => $read_variant:ident => $read:ident => $ty:ty) => {
	    #[doc = "Decodes a `"]
	    #[doc = stringify!($ty)]
	    #[doc = "` from the underlying stream, according to the endianness and numerical encoding in the encoder's context"]
	    pub fn $read(&mut self) -> EncodingResult<$ty> {
		    self.$read_internal(self.ctxt.settings.num_repr.num_encoding, self.ctxt.settings.num_repr.endianness)
	    }
	    
	    fn $read_size(&mut self) -> EncodingResult<$ty> {
		    self.$read_internal(self.ctxt.settings.size_repr.num_encoding, self.ctxt.settings.size_repr.endianness)
	    }
	    
	    fn $read_variant(&mut self) -> EncodingResult<$ty> {
		    self.$read_internal(self.ctxt.settings.variant_repr.num_encoding, self.ctxt.settings.variant_repr.endianness)
	    }
	    
        fn $read_internal(&mut self, num_encoding: NumEncoding, endianness: Endianness) -> EncodingResult<$ty> {
	        Ok(match num_encoding {
		        NumEncoding::Fixed => {
			        let mut bytes: [u8; std::mem::size_of::<$ty>()] = [0u8; std::mem::size_of::<$ty>()];
		            self.stream.read_exact(&mut bytes)?;

		            match endianness {
			            Endianness::BigEndian => <$ty>::from_be_bytes(bytes),
			            Endianness::LittleEndian => <$ty>::from_le_bytes(bytes)
		            }
		        }
		        NumEncoding::Leb128 => {
			        let mut result: $ty = 0;
			        let mut byte = [0u8; 1];
			        let mut shift: u8 = 0;
			        loop {
				        if shift >= <$ty>::BITS as u8 {
					        return Err(EncodingError::VarIntError);
				        }
				        
			            self.stream.read_exact(&mut byte)?;
				        result |= (byte[0] & 0b0111_1111) as $ty << shift;
				        shift += 7;
				        
				        if (byte[0] & 0b1000_0000) == 0 {
					        break;
				        }
					}
			        
			        if shift < <$ty>::BITS as u8 && (byte[0] & 0b0100_0000) != 0 {
				        result |= (!0 << shift);
			        }
			        
			        result
		        }
	        })
        }
    };
}

impl<T: Write> Encoder<'_, T> {
	make_unsigned_write_fn!(_write_u8 => _write_u8_size => _write_u8_variant => write_u8 => u8);
	make_unsigned_write_fn!(_write_u16 => _write_u16_size => _write_u16_variant => write_u16 => u16);
	make_unsigned_write_fn!(_write_u32 => _write_u32_size => _write_u32_variant => write_u32 => u32);
	make_unsigned_write_fn!(_write_u64 => _write_u64_size => _write_u64_variant => write_u64 => u64);
	make_unsigned_write_fn!(_write_u128 => _write_u128_size => _write_u128_variant => write_u128 => u128);
	make_signed_write_fn!(_write_i8 => _write_i8_size => _write_i8_variant => write_i8 => i8);
	make_signed_write_fn!(_write_i16 => _write_i16_size => _write_i16_variant => write_i16 => i16);
	make_signed_write_fn!(_write_i32 => _write_i32_size => _write_i32_variant => write_i32 => i32);
	make_signed_write_fn!(_write_i64 => _write_i64_size => _write_i64_variant => write_i64 => i64);
	make_signed_write_fn!(_write_i128 => _write_i128_size => _write_i128_variant => write_i128 => i128);

	/// Encodes a length. If the flatten attribute is set to Some, this function is a no-op,
	/// otherwise it will behave identically to [`Self::write_usize`].
	pub fn write_length(&mut self, value: usize) -> EncodingResult<()> {
		if self.ctxt.flatten().is_none() {
			self.write_usize(value)?;
		}
		Ok(())
	}

	/// Encodes a `usize` to the underlying stream, according to the endianness,
	/// numerical encoding, bit-width and max size in the encoder's state
	pub fn write_usize(&mut self, value: usize) -> EncodingResult<()> {
		if value > self.ctxt.settings.size_repr.max_size {
			return Err(EncodingError::MaxLengthExceeded {
				max: self.ctxt.settings.size_repr.max_size,
				requested: value
			})
		}
		match self.ctxt.settings.size_repr.width {
			BitWidth::Bit8 => self._write_u8_size(value as _),
			BitWidth::Bit16 => self._write_u16_size(value as _),
			BitWidth::Bit32 => self._write_u32_size(value as _),
			BitWidth::Bit64 => self._write_u64_size(value as _),
			BitWidth::Bit128 => self._write_u128_size(value as _),
		}
	}

	/// Encodes a `isize` to the underlying stream, according to the endianness,
	/// numerical encoding, bit-width and max size in the encoder's state
	pub fn write_isize(&mut self, value: isize) -> EncodingResult<()> {
		if value >= 0 && value as usize > self.ctxt.settings.size_repr.max_size {
			return Err(EncodingError::MaxLengthExceeded {
				max: self.ctxt.settings.size_repr.max_size,
				requested: value as usize
			})
		}
		match self.ctxt.settings.size_repr.width {
			BitWidth::Bit8 => self._write_i8_size(value as _),
			BitWidth::Bit16 => self._write_i16_size(value as _),
			BitWidth::Bit32 => self._write_i32_size(value as _),
			BitWidth::Bit64 => self._write_i64_size(value as _),
			BitWidth::Bit128 => self._write_i128_size(value as _),
		}
	}

	/// Encodes an unsigned enum variant to the underlying stream, according to the endianness,
	/// numerical encoding and bit-width in the encoder's state
	pub fn write_uvariant(&mut self, value: u128) -> EncodingResult<()> {
		match self.ctxt.settings.variant_repr.width {
			BitWidth::Bit8 => self._write_u8_variant(value as _),
			BitWidth::Bit16 => self._write_u16_variant(value as _),
			BitWidth::Bit32 => self._write_u32_variant(value as _),
			BitWidth::Bit64 => self._write_u64_variant(value as _),
			BitWidth::Bit128 => self._write_u128_variant(value as _),
		}
	}

	/// Encodes a signed enum variant to the underlying stream, according to the endianness,
	/// numerical encoding and bit-width in the encoder's state
	pub fn write_ivariant(&mut self, value: i128) -> EncodingResult<()> {
		match self.ctxt.settings.variant_repr.width {
			BitWidth::Bit8 => self._write_i8_variant(value as _),
			BitWidth::Bit16 => self._write_i16_variant(value as _),
			BitWidth::Bit32 => self._write_i32_variant(value as _),
			BitWidth::Bit64 => self._write_i64_variant(value as _),
			BitWidth::Bit128 => self._write_i128_variant(value as _),
		}
	}

	/// Encodes a `bool` to the underlying stream, ignoring any encoding option.
	/// It is guaranteed that, if `value` is `true`, a single u8 will be written to the
	/// underlying stream with the value `1`, and if `value` is `false`, with a value of `0`
	pub fn write_bool(&mut self, value: bool) -> EncodingResult<()> {
		self._write_u8(value as u8, NumEncoding::Fixed, Endianness::LittleEndian)
	}

	/// FIXME Decide how chars should be encoded
	pub fn write_char(&mut self, value: char) -> EncodingResult<()> {
		self.write_u32(value as u32)
	}

	/// Encodes a `f32` to the underlying stream, ignoring the numeric encoding but respecting
	/// the endianness. Equivalent of `Self::write_u32(value.to_bits())` with the numeric
	/// encoding set to Fixed
	pub fn write_f32(&mut self, value: f32) -> EncodingResult<()> {
		self._write_u32(value.to_bits(), NumEncoding::Fixed, self.ctxt.settings.num_repr.endianness)
	}

	/// Encodes a `f64` to the underlying stream, ignoring the numeric encoding but respecting
	/// the endianness. Equivalent of `Self::write_u64(value.to_bits())` with the numeric
	/// encoding set to Fixed
	pub fn write_f64(&mut self, value: f64) -> EncodingResult<()> {
		self._write_u64(value.to_bits(), NumEncoding::Fixed, self.ctxt.settings.num_repr.endianness)
	}

	/// Writes the given slice to the underlying stream as-is.
	pub fn write_raw_bytes(&mut self, bytes: &[u8]) -> EncodingResult<()> {
		Ok(self.stream.write_all(bytes)?)
	}
}

impl<T: Read> Encoder<'_, T> {
	make_unsigned_read_fn!(_read_u8 => _read_u8_size => _read_u8_variant => read_u8 => u8);
	make_unsigned_read_fn!(_read_u16 => _read_u16_size => _read_u16_variant => read_u16 => u16);
	make_unsigned_read_fn!(_read_u32 => _read_u32_size => _read_u32_variant => read_u32 => u32);
	make_unsigned_read_fn!(_read_u64 => _read_u64_size => _read_u64_variant => read_u64 => u64);
	make_unsigned_read_fn!(_read_u128 => _read_u128_size => _read_u128_variant => read_u128 => u128);
	make_signed_read_fn!(_read_i8 => _read_i8_size => _read_i8_variant => read_i8 => i8);
	make_signed_read_fn!(_read_i16 => _read_i16_size => _read_i16_variant => read_i16 => i16);
	make_signed_read_fn!(_read_i32 => _read_i32_size => _read_i32_variant => read_i32 => i32);
	make_signed_read_fn!(_read_i64 => _read_i64_size => _read_i64_variant => read_i64 => i64);
	make_signed_read_fn!(_read_i128 => _read_i128_size => _read_i128_variant => read_i128 => i128);

	/// Decodes a length. If the flatten attribute is set to Some, this function
	/// will return its value, otherwise it will behave identically to [`Self::read_usize`].
	pub fn read_length(&mut self) -> EncodingResult<usize> {
		if let Some(length) = self.ctxt.flatten() {
			Ok(length)
		} else {
			self.read_usize()
		}
	}

	/// Decodes a `usize` from the underlying stream, according to the endianness,
	/// numerical encoding, bit-width and max size in the encoder's state
	pub fn read_usize(&mut self) -> EncodingResult<usize> {
		let value = match self.ctxt.settings.size_repr.width {
			BitWidth::Bit8 => self._read_u8_size()? as usize,
			BitWidth::Bit16 => self._read_u16_size()? as usize,
			BitWidth::Bit32 => self._read_u32_size()? as usize,
			BitWidth::Bit64 => self._read_u64_size()? as usize,
			BitWidth::Bit128 => self._read_u128_size()? as usize,
		};
		if value > self.ctxt.settings.size_repr.max_size {
			return Err(EncodingError::MaxLengthExceeded {
				max: self.ctxt.settings.size_repr.max_size,
				requested: value
			})
		}
		Ok(value)
	}

	/// Decodes a `isize` from the underlying stream, according to the endianness,
	/// numerical encoding, bit-width and max size in the encoder's state
	pub fn read_isize(&mut self) -> EncodingResult<isize> {
		let value = match self.ctxt.settings.size_repr.width {
			BitWidth::Bit8 => self._read_i8_size()? as isize,
			BitWidth::Bit16 => self._read_i16_size()? as isize,
			BitWidth::Bit32 => self._read_i32_size()? as isize,
			BitWidth::Bit64 => self._read_i64_size()? as isize,
			BitWidth::Bit128 => self._read_i128_size()? as isize,
		};
		if value >= 0 && value as usize > self.ctxt.settings.size_repr.max_size {
			return Err(EncodingError::MaxLengthExceeded {
				max: self.ctxt.settings.size_repr.max_size,
				requested: value as usize
			})
		}
		Ok(value)
	}

	/// Decodes an unsigned enum variant from the underlying stream, according to the endianness,
	/// numerical encoding and bit-width in the encoder's state
	pub fn read_uvariant(&mut self) -> EncodingResult<u128> {
		Ok(match self.ctxt.settings.variant_repr.width {
			BitWidth::Bit8 => self._read_u8_variant()? as _,
			BitWidth::Bit16 => self._read_u16_variant()? as _,
			BitWidth::Bit32 => self._read_u32_variant()? as _,
			BitWidth::Bit64 => self._read_u64_variant()? as _,
			BitWidth::Bit128 => self._read_u128_variant()? as _,
		})
	}

	/// Decodes a signed enum variant from the underlying stream, according to the endianness,
	/// numerical encoding and bit-width in the encoder's state
	pub fn read_ivariant(&mut self) -> EncodingResult<i128> {
		Ok(match self.ctxt.settings.variant_repr.width {
			BitWidth::Bit8 => self._read_i8_variant()? as _,
			BitWidth::Bit16 => self._read_i16_variant()? as _,
			BitWidth::Bit32 => self._read_i32_variant()? as _,
			BitWidth::Bit64 => self._read_i64_variant()? as _,
			BitWidth::Bit128 => self._read_i128_variant()? as _,
		})
	}

	/// Decodes a `bool` from the underlying stream, ignoring any encoding option.
	/// It is guaranteed that, one u8 is read from the underlying stream and, if
	/// it's equal to `1`, `true` is returned, if it's equal to `0`, `false` is returned,
	/// if it's equal to any other value, `InvalidBool` error will be returned
	pub fn read_bool(&mut self) -> EncodingResult<bool> {
		match self._read_u8(NumEncoding::Fixed, Endianness::LittleEndian)? {
			0 => Ok(false),
			1 => Ok(true),
			_ => Err(EncodingError::InvalidBool)
		}
	}

	/// FIXME Decide how chars should be decoded
	pub fn read_char(&mut self) -> EncodingResult<char> {
		char::from_u32(self.read_u32()?).ok_or(EncodingError::InvalidChar)
	}

	/// Decodes a `f32` from the underlying stream, ignoring the numeric encoding but respecting
	/// the endianness. Equivalent of `f32::from_bits(self.read_u32())` with the numeric
	/// encoding set to Fixed
	pub fn read_f32(&mut self) -> EncodingResult<f32> {
		Ok(f32::from_bits(self._read_u32(NumEncoding::Fixed, self.ctxt.settings.num_repr.endianness)?))
	}

	/// Decodes a `f64` from the underlying stream, ignoring the numeric encoding but respecting
	/// the endianness. Equivalent of `f64::from_bits(self.read_u64())` with the numeric
	/// encoding set to Fixed
	pub fn read_f64(&mut self) -> EncodingResult<f64> {
		Ok(f64::from_bits(self._read_u64(NumEncoding::Fixed, self.ctxt.settings.num_repr.endianness)?))
	}

	/// Reads `buf.len()` bytes from the stream to the buffer as-is.
	pub fn read_raw_bytes(&mut self, buf: &mut [u8]) -> EncodingResult<()> {
		Ok(self.stream.read_exact(buf)?)
	}
}

/// Represents any kind of error that can happen during encoding and decoding
#[derive(Debug, Error)]
pub enum EncodingError {
	/// Generic IO error
	#[error("IO Error occurred: {0}")]
	IOError(
		#[source]
		#[from]
		io::Error
	),
	/// A var-int was malformed and could not be decoded
	#[error("Malformed var-int encoding")]
	VarIntError,
	/// An invalid character value was read
	#[error("Invalid char value")]
	InvalidChar,
	/// A value other than `1` or `0` was read while decoding a `bool`
	#[error("Invalid bool value")]
	InvalidBool,
	/// Tried to write or read a length greater than the max
	#[error("A length of {requested} exceeded the max allowed value of {max}")]
	MaxLengthExceeded {
		max: usize,
		requested: usize
	},
	/// A string contained invalid UTF8 bytes and couldn't be decoded
	#[error("Invalid string value: {0}")]
	InvalidString(
		#[source]
		#[from]
		FromUtf8Error
	),
	/// A c-like string wasn't null terminated
	#[error("Invalid c-string value: {0}")]
	InvalidCString(
		#[source]
		#[from]
		FromVecWithNulError
	),
	/// Tried to decode an unrecognized enum variant
	#[error("Unrecognized enum variant")]
	InvalidVariant,
	/// A `#[ende(validate = ...)]` check failed
	#[error("Validation error: {0}")]
	ValidationError(String),
	/// A generic serde error occurred
	#[cfg(feature = "serde")]
	#[error("Serde error occurred: {0}")]
	SerdeError(String),
	/// A cryptographic error occurred
	#[cfg(feature = "encryption")]
	#[error("Cryptographic error: {0}")]
	EncryptionError(
		#[source]
		#[from]
		encryption::CryptoError
	),
	/// A compression error occurred
	#[cfg(feature = "compression")]
	#[error("Compression error: {0}")]
	CompressionError(
		#[source]
		#[from]
		compression::CompressionError
	)
}

/// A convenience alias to `Result<T, EncodingError>`
pub type EncodingResult<T> = Result<T, EncodingError>;

/// The base trait for anything that can be Encoded.
/// Indicates that a type can be converted into a sequence of bytes
pub trait Encode {
	/// Encodes `self` into a binary format.<br>
	/// If the result is Ok,
	/// implementations should guarantee that the state of the encoder
	/// is the same as before calling this function. If the result is Err,
	/// no guarantees should be made about the state of the encoder,
	/// and users should reset it before reuse.<br>
	/// Implementation are discouraged from writing `encode` implementations
	/// that modify `self` through interior mutability
	fn encode<T: Write>(&self, encoder: &mut Encoder<T>) -> EncodingResult<()>;
}

/// The base trait for anything that can be Decoded.
/// Indicates that a sequence of bytes can be converted back into a type
pub trait Decode: Sized {
	/// Decodes `Self` from a binary format.<br>
	/// If the result is Ok,
	/// implementations should guarantee that the state of the encoder
	/// is the same as before calling this function. If the result is Err,
	/// no guarantees should be made about the state of the encoder,
	/// and users should reset it before reuse.<br>
	fn decode<T: Read>(decoder: &mut Encoder<T>) -> EncodingResult<Self>;
}

// Primitives

macro_rules! impl_primitives {
    ($($ty:ty => $write:ident => $read:ident);* $(;)? ) => {
	    $(
	    impl Encode for $ty {
		    fn encode<T: Write>(&self, encoder: &mut Encoder<T>) -> EncodingResult<()> {
		        encoder.$write(*self)
		    }
	    }
	    impl Decode for $ty {
		    fn decode<T: Read>(decoder: &mut Encoder<T>) -> EncodingResult<Self> where Self: Sized {
		        decoder.$read()
		    }
	    }
	    )*
    };
}

impl_primitives!{
	u8 => write_u8 => read_u8;
	u16 => write_u16 => read_u16;
	u32 => write_u32 => read_u32;
	u64 => write_u64 => read_u64;
	u128 => write_u128 => read_u128;
	i8 => write_i8 => read_i8;
	i16 => write_i16 => read_i16;
	i32 => write_i32 => read_i32;
	i64 => write_i64 => read_i64;
	i128 => write_i128 => read_i128;
	bool => write_bool => read_bool;
	char => write_char => read_char;
	f32 => write_f32 => read_f32;
	f64 => write_f64 => read_f64;
	usize => write_usize => read_usize;
	isize => write_isize => read_isize;
}

// STRINGS

impl Encode for String {
	fn encode<T: Write>(&self, encoder: &mut Encoder<T>) -> EncodingResult<()> {
		encoder.write_length(self.len())?;
		encoder.write_raw_bytes(self.as_bytes())
	}
}

impl Decode for String {
	fn decode<T: Read>(decoder: &mut Encoder<T>) -> EncodingResult<Self> where Self: Sized {
		let len = decoder.read_length()?;
		let mut buffer = vec![0u8; len];
		decoder.read_raw_bytes(&mut buffer)?;
		Ok(String::from_utf8(buffer)?)
	}
}

impl Encode for &str {
	fn encode<T: Write>(&self, encoder: &mut Encoder<T>) -> EncodingResult<()> {
		encoder.write_length(self.len())?;
		encoder.write_raw_bytes(self.as_bytes())
	}
}

// CSTRING

impl Encode for CString {
	fn encode<T: Write>(&self, encoder: &mut Encoder<T>) -> EncodingResult<()> {
		if encoder.ctxt.flatten().is_some() {
			encoder.write_raw_bytes(self.as_bytes())
		} else {
			encoder.write_raw_bytes(self.as_bytes_with_nul())
		}
	}
}

impl Decode for CString {
	fn decode<T: Read>(decoder: &mut Encoder<T>) -> EncodingResult<Self> where Self: Sized {
		if let Some(length) = decoder.ctxt.flatten() {
			let mut buffer = vec![0; length + 1];
			decoder.read_raw_bytes(&mut buffer[..length])?;
			Ok(CString::from_vec_with_nul(buffer)?)
		} else {
			let mut last_byte: u8;
			let mut buffer = Vec::new();
			while { last_byte = decoder.read_u8()?; last_byte != 0 } {
				buffer.push(last_byte);
			}
			buffer.push(0u8);
			Ok(CString::from_vec_with_nul(buffer)?)
		}
	}
}

impl Encode for CStr {
	fn encode<T: Write>(&self, encoder: &mut Encoder<T>) -> EncodingResult<()> {
		if encoder.ctxt.flatten().is_some() {
			encoder.write_raw_bytes(self.to_bytes())
		} else {
			encoder.write_raw_bytes(self.to_bytes_with_nul())
		}
	}
}

// Option

impl<T: Encode> Encode for Option<T> {
	fn encode<G: Write>(&self, encoder: &mut Encoder<G>) -> EncodingResult<()> {
		if encoder.ctxt.flatten().is_some() {
			match self {
				None => Ok(()),
				Some(x) => {
					x.encode(encoder)
				}
			}
		} else {
			match self {
				None => encoder.write_bool(false),
				Some(value) => {
					encoder.write_bool(true)?;
					value.encode(encoder)
				}
			}
		}
	}
}

impl<T: Decode> Decode for Option<T> {
	fn decode<G: Read>(decoder: &mut Encoder<G>) -> EncodingResult<Self> where Self: Sized {
		if decoder.ctxt.flatten().is_some() {
			Ok(Some(T::decode(decoder)?))
		} else {
			Ok(match decoder.read_bool()? {
				true => Some(T::decode(decoder)?),
				false => None
			})
		}
	}
}

// Slice

impl<T: Encode> Encode for &[T] {
	fn encode<G: Write>(&self, encoder: &mut Encoder<G>) -> EncodingResult<()> {
		encoder.write_length(self.len())?;
		for i in 0..self.len() {
			self[i].encode(encoder)?;
		}
		Ok(())
	}
}

impl<T: Encode, const SIZE: usize> Encode for [T; SIZE] {
	fn encode<G: Write>(&self, encoder: &mut Encoder<G>) -> EncodingResult<()> {
		for i in 0..SIZE {
			self[i].encode(encoder)?;
		}
		Ok(())
	}
}

impl<T: Decode + Default, const SIZE: usize> Decode for [T; SIZE] {
	fn decode<G: Read>(decoder: &mut Encoder<G>) -> EncodingResult<Self> where Self: Sized {
		let mut uninit = array_init(|_| T::decode(decoder).unwrap());
		for i in 0..SIZE {
			uninit[i] = T::decode(decoder)?;
		}
		Ok(uninit)
	}
}

// Vec

impl<T: Encode> Encode for Vec<T> {
	fn encode<G: Write>(&self, encoder: &mut Encoder<G>) -> EncodingResult<()> {
		Encode::encode(&self.as_slice(), encoder)
	}
}

impl<T: Decode> Decode for Vec<T> {
	fn decode<G: Read>(decoder: &mut Encoder<G>) -> EncodingResult<Self> where Self: Sized {
		let size = decoder.read_length()?;
		let mut vec = Vec::with_capacity(size);
		for _ in 0..size {
			vec.push(Decode::decode(decoder)?);
		}
		Ok(vec)
	}
}

// Maps

impl<K: Encode, V: Encode> Encode for HashMap<K, V> {
	fn encode<T: Write>(&self, encoder: &mut Encoder<T>) -> EncodingResult<()> {
		encoder.write_length(self.len())?;
		for (k, v) in self.iter() {
			k.encode(encoder)?;
			v.encode(encoder)?;
		}
		Ok(())
	}
}

impl<K: Decode + Eq + Hash, V: Decode> Decode for HashMap<K, V> {
	fn decode<T: Read>(decoder: &mut Encoder<T>) -> EncodingResult<Self> where Self: Sized {
		let size = decoder.read_length()?;
		let mut map = HashMap::with_capacity(size);
		for _ in 0..size {
			map.insert(K::decode(decoder)?, V::decode(decoder)?);
		}
		Ok(map)
	}
}

// Phantom data

impl<T> Encode for PhantomData<T> {
	fn encode<G: Write>(&self, _encoder: &mut Encoder<G>) -> EncodingResult<()> {
		Ok(())
	}
}

impl<T> Decode for PhantomData<T> {
	fn decode<G: Read>(_decoder: &mut Encoder<G>) -> EncodingResult<Self> where Self: Sized {
		Ok(Self)
	}
}

// Unit

impl Encode for () {
	fn encode<T: Write>(&self, _encoder: &mut Encoder<T>) -> EncodingResult<()> {
		Ok(())
	}
}

impl Decode for () {
	fn decode<T: Read>(_decoder: &mut Encoder<T>) -> EncodingResult<Self> where Self: Sized {
		Ok(())
	}
}

// Some tuples

macro_rules! consume {
    ($x:tt, $expr:expr) => {
	    $expr
    };
}

macro_rules! tuple_impl {
    ($($name:ident)+) => {
	    #[allow(non_snake_case)]
	    impl<$($name: $crate::Encode),+> $crate::Encode for ($($name),+) {
		    fn encode<__T: Write>(&self, encoder: &mut Encoder<__T>) -> EncodingResult<()> {
		        let ($($name),*) = self;
			    $(
			        $crate::Encode::encode($name, encoder)?;
			    )+
			    Ok(())
		    }
	    }

	    #[allow(non_snake_case)]
	    impl<$($name: $crate::Decode),+> $crate::Decode for ($($name),+) {
		    fn decode<__T: Read>(decoder: &mut Encoder<__T>) -> EncodingResult<Self> where Self: Sized {
			    Ok(($(
		            consume!($name, $crate::Decode::decode(decoder)?),
		        )+))
		    }
	    }
    };
}

tuple_impl! { A B }
tuple_impl! { A B C }
tuple_impl! { A B C D }
tuple_impl! { A B C D E }
tuple_impl! { A B C D E F }
tuple_impl! { A B C D E F G }
tuple_impl! { A B C D E F G H }
tuple_impl! { A B C D E F G H I }
tuple_impl! { A B C D E F G H I J }
tuple_impl! { A B C D E F G H I J K }
tuple_impl! { A B C D E F G H I J K L }
tuple_impl! { A B C D E F G H I J K L M }
tuple_impl! { A B C D E F G H I J K L M N }
tuple_impl! { A B C D E F G H I J K L M N O }
tuple_impl! { A B C D E F G H I J K L M N O P } // Up to 16