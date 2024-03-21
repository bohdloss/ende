use std::fmt::Display;
use std::io::{Read, Write};
use serde::{de, Deserializer, ser, Serialize, Serializer};
use serde::ser::{SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple, SerializeTupleStruct, SerializeTupleVariant};
use serde::de::{DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};
use crate::{BinStream, Decode, Encode, EncodingError};

impl ser::Error for EncodingError {
	fn custom<T>(msg: T) -> Self where T: Display {
		Self::SerdeError(msg.to_string())
	}
}

impl de::Error for EncodingError {
	fn custom<T>(msg: T) -> Self where T: Display {
		Self::SerdeError(msg.to_string())
	}
}

impl<T: Write> Serializer for &mut BinStream<T> {
	type Ok = ();
	type Error = EncodingError;
	type SerializeSeq = Self;
	type SerializeTuple = Self;
	type SerializeTupleStruct = Self;
	type SerializeTupleVariant = Self;
	type SerializeMap = Self;
	type SerializeStruct = Self;
	type SerializeStructVariant = Self;

	fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
		self.write_bool(v)
	}

	fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
		self.write_i8(v)
	}

	fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
		self.write_i16(v)
	}

	fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
		self.write_i32(v)
	}

	fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
		self.write_i64(v)
	}

	fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
		self.write_i128(v)
	}

	fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
		self.write_u8(v)
	}

	fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
		self.write_u16(v)
	}

	fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
		self.write_u32(v)
	}

	fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
		self.write_u64(v)
	}

	fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
		self.write_u128(v)
	}

	fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
		self.write_f32(v)
	}

	fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
		self.write_f64(v)
	}

	fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
		self.write_char(v)
	}

	fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
		v.encode(self)
	}

	fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
		self.write_raw_bytes(v)
	}

	fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
		if self.options.flatten().is_none() {
			self.write_bool(false)?;
		}
		Ok(())
	}

	fn serialize_some<G: ?Sized>(self, value: &G) -> Result<Self::Ok, Self::Error> where G: Serialize {
		if self.options.flatten().is_none() {
			self.write_bool(true)?;
		}
		value.serialize(self)
	}

	fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
		Ok(())
	}

	fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
		Ok(())
	}

	fn serialize_unit_variant(self, _name: &'static str, variant_index: u32, _variant: &'static str) -> Result<Self::Ok, Self::Error> {
		self.write_uvariant(variant_index as u128)
	}

	fn serialize_newtype_struct<G: ?Sized>(self, _name: &'static str, value: &G) -> Result<Self::Ok, Self::Error> where G: Serialize {
		value.serialize(self)
	}

	fn serialize_newtype_variant<G: ?Sized>(self, _name: &'static str, variant_index: u32, _variant: &'static str, value: &G) -> Result<Self::Ok, Self::Error> where G: Serialize {
		self.write_uvariant(variant_index as u128)?;
		value.serialize(self)
	}

	fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
		let len = len.ok_or(EncodingError::SerdeError("Length must be known upfront".to_string()))?;
		self.write_length(len)?;
		Ok(self)
	}

	fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
		Ok(self)
	}

	fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
		Ok(self)
	}

	fn serialize_tuple_variant(self, _name: &'static str, variant_index: u32, _variant: &'static str, _len: usize) -> Result<Self::SerializeTupleVariant, Self::Error> {
		self.write_uvariant(variant_index as u128)?;
		Ok(self)
	}

	fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
		let len = len.ok_or(EncodingError::SerdeError("Length must be known upfront".to_string()))?;
		self.write_length(len)?;
		Ok(self)
	}

	fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct, Self::Error> {
		Ok(self)
	}

	fn serialize_struct_variant(self, _name: &'static str, variant_index: u32, _variant: &'static str, _len: usize) -> Result<Self::SerializeStructVariant, Self::Error> {
		self.write_uvariant(variant_index as u128)?;
		Ok(self)
	}

	fn is_human_readable(&self) -> bool {
		false
	}
}

impl<T: Write> SerializeSeq for &mut BinStream<T> {
	type Ok = ();
	type Error = EncodingError;

	fn serialize_element<G: ?Sized>(&mut self, value: &G) -> Result<(), Self::Error> where G: Serialize {
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Ok(())
	}
}

impl<T: Write> SerializeTuple for &mut BinStream<T> {
	type Ok = ();
	type Error = EncodingError;

	fn serialize_element<G: ?Sized>(&mut self, value: &G) -> Result<(), Self::Error> where G: Serialize {
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Ok(())
	}
}

impl<T: Write> SerializeTupleStruct for &mut BinStream<T> {
	type Ok = ();
	type Error = EncodingError;

	fn serialize_field<G: ?Sized>(&mut self, value: &G) -> Result<(), Self::Error> where G: Serialize {
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Ok(())
	}
}

impl<T: Write> SerializeTupleVariant for &mut BinStream<T> {
	type Ok = ();
	type Error = EncodingError;

	fn serialize_field<G: ?Sized>(&mut self, value: &G) -> Result<(), Self::Error> where G: Serialize {
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Ok(())
	}
}

impl<T: Write> SerializeMap for &mut BinStream<T> {
	type Ok = ();
	type Error = EncodingError;

	fn serialize_key<G: ?Sized>(&mut self, key: &G) -> Result<(), Self::Error> where G: Serialize {
		key.serialize(&mut **self)
	}

	fn serialize_value<G: ?Sized>(&mut self, value: &G) -> Result<(), Self::Error> where G: Serialize {
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Ok(())
	}
}

impl<T: Write> SerializeStruct for &mut BinStream<T> {
	type Ok = ();
	type Error = EncodingError;

	fn serialize_field<G: ?Sized>(&mut self, _key: &'static str, value: &G) -> Result<(), Self::Error> where G: Serialize {
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Ok(())
	}
}

impl<T: Write> SerializeStructVariant for &mut BinStream<T> {
	type Ok = ();
	type Error = EncodingError;

	fn serialize_field<G: ?Sized>(&mut self, _key: &'static str, value: &G) -> Result<(), Self::Error> where G: Serialize {
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Ok(())
	}
}

impl<'de, T: Read> Deserializer<'de> for &mut BinStream<T> {
	type Error = EncodingError;

	fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		Err(EncodingError::SerdeError("deserialize_any: This data format is non-describing".to_string()))
	}

	fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_bool(self.read_bool()?)
	}

	fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_i8(self.read_i8()?)
	}

	fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_i16(self.read_i16()?)
	}

	fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_i32(self.read_i32()?)
	}

	fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_i64(self.read_i64()?)
	}

	fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_i128(self.read_i128()?)
	}

	fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_u8(self.read_u8()?)
	}

	fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_u16(self.read_u16()?)
	}

	fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_u32(self.read_u32()?)
	}

	fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_u64(self.read_u64()?)
	}

	fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_u128(self.read_u128()?)
	}

	fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_f32(self.read_f32()?)
	}

	fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_f64(self.read_f64()?)
	}

	fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_char(self.read_char()?)
	}

	fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_str(&String::decode(self)?)
	}

	fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_string(String::decode(self)?)
	}

	fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_bytes(Vec::decode(self)?.as_slice())
	}

	fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_byte_buf(Vec::decode(self)?)
	}

	fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		if self.options.flatten().is_some() || self.read_bool()? {
			visitor.visit_some(self)
		} else {
			visitor.visit_none()
		}
	}

	fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_unit()
	}

	fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_unit()
	}

	fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_newtype_struct(self)
	}

	fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		let len = self.read_length()?;
		visitor.visit_seq(BinSeqDeserializer {
			stream: self,
			len,
		})
	}

	fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_seq(BinSeqDeserializer {
			stream: self,
			len,
		})
	}

	fn deserialize_tuple_struct<V>(self, _name: &'static str, len: usize, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_seq(BinSeqDeserializer {
			stream: self,
			len,
		})
	}

	fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		let len = self.read_length()?;
		visitor.visit_map(BinMapDeserializer {
			stream: self,
			len
		})
	}

	fn deserialize_struct<V>(self, _name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_seq(BinSeqDeserializer {
			stream: self,
			len: fields.len(),
		})
	}

	fn deserialize_enum<V>(self, _name: &'static str, _variants: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_enum(self)
	}

	fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		visitor.visit_u32(self.read_uvariant()? as u32)
	}

	fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		Err(EncodingError::SerdeError("deserialize_ignored_any: This data format is non-describing".to_string()))
	}

	fn is_human_readable(&self) -> bool {
		false
	}
}

struct BinSeqDeserializer<'a, T: Read> {
	stream: &'a mut BinStream<T>,
	len: usize
}

impl<'de, 'a, T: Read> SeqAccess<'de> for BinSeqDeserializer<'a, T> {
	type Error = EncodingError;

	fn next_element_seed<G>(&mut self, seed: G) -> Result<Option<G::Value>, Self::Error> where G: DeserializeSeed<'de> {
		if self.len != 0 {
			seed.deserialize(&mut *self.stream).map(Some)
		} else {
			Ok(None)
		}
	}

	fn size_hint(&self) -> Option<usize> {
		Some(self.len)
	}
}

struct BinMapDeserializer<'a, T: Read> {
	stream: &'a mut BinStream<T>,
	len: usize
}

impl<'de, 'a, T: Read> MapAccess<'de> for BinMapDeserializer<'a, T> {
	type Error = EncodingError;

	fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error> where K: DeserializeSeed<'de> {
		if self.len != 0 {
			self.len -= 1;
			seed.deserialize(&mut *self.stream).map(Some)
		} else {
			Ok(None)
		}
	}

	fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error> where V: DeserializeSeed<'de> {
		seed.deserialize(&mut *self.stream)
	}

	fn size_hint(&self) -> Option<usize> {
		Some(self.len)
	}
}

impl<'de, T: Read> EnumAccess<'de> for &mut BinStream<T> {
	type Error = EncodingError;
	type Variant = Self;

	fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error> where V: DeserializeSeed<'de> {
		let val = seed.deserialize(&mut *self)?;
		Ok((val, self))
	}
}

impl<'de, T: Read> VariantAccess<'de> for &mut BinStream<T> {
	type Error = EncodingError;

	fn unit_variant(self) -> Result<(), Self::Error> {
		Ok(())
	}

	fn newtype_variant_seed<G>(self, seed: G) -> Result<G::Value, Self::Error> where G: DeserializeSeed<'de> {
		seed.deserialize(self)
	}

	fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		self.deserialize_tuple(len, visitor)
	}

	fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error> where V: Visitor<'de> {
		self.deserialize_struct("", fields, visitor)
	}
}