#![allow(unused)]

use alloc::borrow::Cow;
use uuid::Uuid;
use crate::facade::fake::*;

use ende_derive::{BorrowDecode, Decode, Encode};

#[derive(BorrowDecode)]
pub struct Borrow<'a, 'b> {
    string: &'a str,
    #[ende(borrow)]
    data: &'b [u8],
}

#[derive(Encode, Decode)]
#[ende(num: little_endian)]
#[allow(unused)]
#[cfg(feature = "std")]
pub struct StructStruct {
    #[ende(skip)]
    #[ende(default: 100)]
    value1: u64,
    #[ende(if: *value1 > 10)]
    #[ende(default: *value1 as f32)]
    #[ende(as: u8)]
    value2: f32,
    unit: (),
    #[ende(with: rsa(&[], &[]))]
    secret: Vec<u8>,
}

#[derive(Encode, Decode)]
struct MyStruct {
    secret_key: Vec<u8>,
    iv: Vec<u8>,
    /// While **encoding**, this field is compressed -> encrypted.
    /// While **decoding**, this field is decrypted -> decompressed.
    #[ende(redir: gzip(9))]
    #[ende(redir: aes(iv, secret_key))]
    super_secret_data: Vec<u8>,
    file_pointer: usize,
    /// Marks the current offset, seeks to `file_pointer` bytes from the start of the file,
    /// encodes/decodes the field, then seeks back.
    #[ende(ptr start: * file_pointer)]
    apple_count: u64,
    /// This field is effectively laid *right after* `file_pointer`
    /// in the binary representation.
    other_data: i32,
}

#[derive(Encode, Decode)]
pub struct TupleStruct(
    u64,
    #[ende(num: leb128, big_endian; size: max = 15, bit16, little_endian, fixed; variant: bit128, fixed, big_endian)]
     f64,
);

#[derive(Encode, Decode)]
pub struct UnitStruct;

#[derive(Encode, Decode)]
#[repr(isize)]
#[ende(size: bit64; variant: bit64, leb128)]
pub enum Enum {
    StructVariant {
        value1: u64,
        #[ende(validate: *value2 == 16.0, "Expected 16.0, got {}", value2)]
        value2: f64,
    },
    TupleVariant(u64, #[ende(if: *m0 >= 154)] f64) = u16::MAX as isize,
    UnitVariant,
}

#[derive(Encode, Decode)]
pub struct EmptyStructStruct {}

#[derive(Encode, Decode)]
pub struct EmptyTupleStruct();

#[derive(Encode, Decode)]
#[ende(variant: little_endian)]
pub enum EmptyVariantEnum {
    StructVariant {},
    TupleVariant(),
}

#[derive(Encode, Decode)]
pub enum EmptyEnum {}

// #[derive(Encode, Decode)]
// pub union Union {
// 	value1: u64,
// 	value2: f64
// }
//
// Will yield the following errors:
//
// error: Encode cannot be derived on unions
//   --> src/test.rs:42:5
//    |
// 42 | pub union Union {
//    |     ^^^^^
//
// error: Decode cannot be derived on unions
//   --> src/test.rs:42:5
//    |
// 42 | pub union Union {
//    |     ^^^^^

#[derive(Encode, Decode)]
#[ende(with: aes)]
#[cfg(feature = "std")]
pub struct VersionContainer {
    name_present: bool,
    #[ende(if: *name_present)]
    name: String,
}

#[test]
pub fn test() {
    // let mut mem = [0u8; 1024];
    // let mut options = BinSettings::default();
    // options.num_repr.num_encoding = NumEncoding::Leb128;
    // let mut stream = Encoder::new(&mut mem, Context::with_options(options));
    //
    // let orig = i128::MIN;
    // println!("{:#0130b}", orig);
    // println!("{orig}");
    // stream.write_i128(orig).unwrap();
    // stream.stream.flush().unwrap();
    // stream.stream.rewind().unwrap();
    // let val = stream.read_i128().unwrap();
    // println!("{:#0130b}", val);
    // println!("{val}");
}
