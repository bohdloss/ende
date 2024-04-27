use crate::{BitWidth, Endianness, NumEncoding, StrEncoding};
use core::fmt;
use embedded_io::{Error, ErrorKind, ReadExactError};
use parse_display::Display;

macro_rules! impl_error {
    ($name:ident) => {
        #[cfg(feature = "unstable")]
        impl core::error::Error for $name {}

        #[cfg(all(not(feature = "unstable"), feature = "std"))]
        impl std::error::Error for $name {}
    };
}

/// Represents any kind of error that can happen during encoding and decoding
#[derive(Debug, Display)]
pub enum EncodingError {
    /// Generic IO error
    #[display("IO Error occurred: {:0?}")]
    IOError(ErrorKind),
    /// The end of the file or buffer was reached but more data was expected
    #[display("Unexpected end of file/buffer")]
    UnexpectedEnd,
    /// A var-int was malformed and could not be decoded
    #[display("Malformed var-int encoding")]
    VarIntError,
    /// An invalid character value was read
    #[display("Invalid char value")]
    InvalidChar,
    /// A value other than `1` or `0` was read while decoding a `bool`
    #[display("Invalid bool value")]
    InvalidBool,
    /// An attempt was made to encode or decode a string, but *something* went wrong.
    #[display("String error: {0}")]
    StringError(StringError),
    /// Tried to write or read a length greater than the max
    #[display("A length of {requested} exceeded the max allowed value of {max}")]
    MaxLengthExceeded { max: usize, requested: usize },
    /// Tried to decode an unrecognized enum variant
    #[display("Unrecognized enum variant")]
    InvalidVariant,
    /// An attempt was made to flatten an option or result, but the inner value was unexpected.
    /// Example: `#[ende(flatten: some)]` applied on an `Option` containing the `None` variant
    #[display("Flatten error: {0}")]
    FlattenError(FlattenError),
    /// An attempt was made to lock a RefCell/Mutex/RwLock or similar, but it failed.
    #[display("Lock error: couldn't lock a RefCell/Mutex/RwLock or similar")]
    LockError,
    /// A piece of data couldn't be borrowed from the encoder. This is a recoverable error,
    /// meaning the decoding operation can be attempted again with a non-borrowing function.
    #[display("Borrow error: {0}")]
    BorrowError(BorrowError),
    /// A `#[ende(validate = ...)]` check failed
    #[display("Validation error: {0}")]
    ValidationError(
        #[cfg(feature = "alloc")] alloc::string::String,
        #[cfg(not(feature = "alloc"))] &'static str,
    ),
    /// A generic serde error occurred
    #[cfg(all(feature = "serde", feature = "alloc"))]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "serde")))]
    #[display("Serde error: {0}")]
    SerdeError(alloc::string::String),
    /// A generic serde error occurred
    #[cfg(all(feature = "serde", not(feature = "alloc")))]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "serde")))]
    #[display("Serde error")]
    SerdeError,
}

impl EncodingError {
    pub fn validation_error<'a>(fmt: fmt::Arguments<'a>) -> Self {
        #[cfg(feature = "alloc")]
        #[allow(unused_imports)]
        {
            use alloc::string::ToString;
            Self::ValidationError(fmt.to_string())
        }
        #[cfg(not(feature = "alloc"))]
        {
            if let Some(str) = fmt.as_str() {
                Self::ValidationError(str)
            } else {
                Self::ValidationError("Unknown")
            }
        }
    }
}

impl Error for EncodingError {
    fn kind(&self) -> ErrorKind {
        match self {
            EncodingError::IOError(io_error) => io_error.kind().into(),
            EncodingError::UnexpectedEnd => ErrorKind::Other,
            EncodingError::FlattenError(_) => ErrorKind::InvalidInput,
            EncodingError::LockError => ErrorKind::Other,
            EncodingError::BorrowError(_) => ErrorKind::Other,
            #[cfg(all(feature = "serde", feature = "alloc"))]
            EncodingError::SerdeError(_) => ErrorKind::Other,
            #[cfg(all(feature = "serde", not(feature = "alloc")))]
            EncodingError::SerdeError => ErrorKind::Other,
            _ => ErrorKind::InvalidData,
        }
    }
}

impl_error!(EncodingError);

#[cfg(feature = "std")]
impl From<std::io::Error> for EncodingError {
    fn from(value: std::io::Error) -> Self {
        match value.kind() {
            std::io::ErrorKind::UnexpectedEof => Self::UnexpectedEnd,
            kind @ _ => Self::IOError(kind.into()),
        }
    }
}

impl From<ErrorKind> for EncodingError {
    fn from(value: ErrorKind) -> Self {
        Self::IOError(value)
    }
}

impl<T: Error> From<ReadExactError<T>> for EncodingError {
    fn from(value: ReadExactError<T>) -> Self {
        match value {
            ReadExactError::UnexpectedEof => Self::UnexpectedEnd,
            ReadExactError::Other(io_error) => Self::IOError(io_error.kind().into()),
        }
    }
}

impl From<StringError> for EncodingError {
    fn from(value: StringError) -> Self {
        Self::StringError(value)
    }
}

impl From<FlattenError> for EncodingError {
    fn from(value: FlattenError) -> Self {
        Self::FlattenError(value)
    }
}

impl From<BorrowError> for EncodingError {
    fn from(value: BorrowError) -> Self {
        Self::BorrowError(value)
    }
}

/// Represents an error occurred while encoding or decoding a string, including intermediate
/// conversion errors and the presence of null bytes in unexpected scenarios.
#[derive(Debug, Display)]
pub enum StringError {
    /// A generic conversion error. E.G. converting an `OsStr` to `str` and back
    #[display("String conversion error")]
    ConversionError,
    /// A string contained non-ascii characters
    #[display("Invalid ASCII characters in string data")]
    InvalidAscii,
    /// A string couldn't be converted to-from utf8 (necessary step for the rust string type)
    #[display("Invalid UTF-8 characters in string data")]
    InvalidUtf8,
    /// A string contained invalid UTF-16 data
    #[display("Invalid UTF-16 characters in string data")]
    InvalidUtf16,
    /// A string contained invalid UTF-32 data
    #[display("Invalid UTF-32 characters in string data")]
    InvalidUtf32,
    /// A c-like string contained zeroes
    #[display("Null-terminated string contained a null *inside*")]
    InvalidCString,
}

impl_error!(StringError);

/// Represents an error related to the "flatten" functionality, with potentially useful diagnostics
#[derive(Debug, Display)]
pub enum FlattenError {
    /// A value other than `1` or `0` was read from the `flatten` state variable
    #[display("Invalid bool value")]
    InvalidBool,
    #[display("Boolean state mismatch: expected {expected}, got {got}")]
    BoolMismatch { expected: bool, got: bool },
    #[display("Length mismatch: expected {expected}, got {got}")]
    LenMismatch { expected: usize, got: usize },
}

impl_error!(FlattenError);

#[derive(Debug, Display)]
pub enum BorrowError {
    #[display("This type doesn't support zero-copy decoding")]
    Unsupported,
    #[display(
        "String encoding mismatch: expected {found} while decoding a {while_decoding} string"
    )]
    StrEncodingMismatch {
        found: StrEncoding,
        while_decoding: StrEncoding,
    },
    #[display("Endianness mismatch: stream contains {found} data, but system uses {system}")]
    EndiannessMismatch {
        found: Endianness,
        system: Endianness,
    },
    #[display("Bit width mismatch: stream contains {found} data, but system uses {system}")]
    BitWidthMismatch { found: BitWidth, system: BitWidth },
    #[display("Non-borrowable numerical encoding: {num_encoding} can't be directly borrowed")]
    NonBorrowableNumEncoding { num_encoding: NumEncoding },
    #[display(
        "Alignment mismatch: borrowing this data requires its alignment to match the system's"
    )]
    AlignmentMismatch,
}

impl_error!(BorrowError);

/// A convenience alias to `Result<T, EncodingError>`
pub type EncodingResult<T> = Result<T, EncodingError>;
