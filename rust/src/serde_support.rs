//! Optional serde integration.

use crate::{Error, Map, Value};
use serde::de::{
    self, DeserializeOwned, Deserializer, EnumAccess, IntoDeserializer, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};
use serde::ser::{
    Impossible, Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, Serializer,
};
use std::fmt;

/// Deserializes a handlist document into a Rust type.
pub fn from_str<T>(source: &str) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let value = crate::parse(source)?;
    T::deserialize(value)
}

/// Serializes any `serde::Serialize` value into a handlist [`Value`].
pub fn to_value<T>(value: &T) -> Result<Value, Error>
where
    T: ?Sized + Serialize,
{
    value.serialize(ValueSerializer)
}

/// Serializes any `serde::Serialize` value into canonical handlist text.
pub fn to_string<T>(value: &T) -> Result<String, Error>
where
    T: ?Sized + Serialize,
{
    let value = to_value(value)?;
    crate::to_string_value(&value)
}

impl<'de> Deserializer<'de> for Value {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self {
            Value::Null => visitor.visit_unit(),
            Value::Bool(value) => visitor.visit_bool(value),
            Value::Int(value) => visitor.visit_i64(value),
            Value::Float(value) => visitor.visit_f64(value),
            Value::String(value) => visitor.visit_string(value),
            Value::Seq(values) => visitor.visit_seq(SeqValues {
                iter: values.into_iter(),
            }),
            Value::Map(map) => visitor.visit_map(MapValues {
                iter: map.into_iter(),
                value: None,
            }),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self {
            Value::Null => visitor.visit_none(),
            other => visitor.visit_some(other),
        }
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self {
            Value::String(variant) => visitor.visit_enum(EnumValue {
                variant,
                value: None,
            }),
            Value::Map(map) => {
                let mut iter = map.into_iter();
                let Some((variant, value)) = iter.next() else {
                    return Err(Error::new(
                        "expected a non-empty single-entry mapping for an enum",
                    ));
                };
                if iter.next().is_some() {
                    return Err(Error::new(
                        "expected a single-entry mapping for an enum variant",
                    ));
                }
                visitor.visit_enum(EnumValue {
                    variant,
                    value: Some(value),
                })
            }
            other => Err(Error::new(format!(
                "expected an enum string or single-entry mapping, found {}",
                other.kind()
            ))),
        }
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self {
            Value::String(text) => {
                let mut chars = text.chars();
                match (chars.next(), chars.next()) {
                    (Some(ch), None) => visitor.visit_char(ch),
                    _ => Err(Error::new("expected a single-character string")),
                }
            }
            other => other.deserialize_any(visitor),
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
    }
}

struct SeqValues {
    iter: std::vec::IntoIter<Value>,
}

impl<'de> SeqAccess<'de> for SeqValues {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(value) => seed.deserialize(value).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

struct MapValues {
    iter: std::vec::IntoIter<(String, Value)>,
    value: Option<Value>,
}

impl<'de> MapAccess<'de> for MapValues {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(MapKey { value: &key }).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        match self.value.take() {
            Some(value) => seed.deserialize(value),
            None => Err(Error::new("mapping value requested before its key")),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

/// A mapping key deserializer.
///
/// handlist keys are always strings, but downstream types (for example
/// `BTreeMap<u8, String>`) may expect numeric or boolean keys. Like
/// `serde_json`, this deserializer parses the string when the target type asks
/// for a number or a boolean.
struct MapKey<'a> {
    value: &'a str,
}

macro_rules! deserialize_key_number {
    ($method:ident, $visit:ident, $ty:ty) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            let parsed = self.value.parse::<$ty>().map_err(|_| {
                Error::new(format!(
                    "mapping key {:?} is not a valid {}",
                    self.value,
                    stringify!($ty)
                ))
            })?;
            visitor.$visit(parsed)
        }
    };
}

impl<'de> Deserializer<'de> for MapKey<'_> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.value)
    }

    deserialize_key_number!(deserialize_i8, visit_i8, i8);
    deserialize_key_number!(deserialize_i16, visit_i16, i16);
    deserialize_key_number!(deserialize_i32, visit_i32, i32);
    deserialize_key_number!(deserialize_i64, visit_i64, i64);
    deserialize_key_number!(deserialize_i128, visit_i128, i128);
    deserialize_key_number!(deserialize_u8, visit_u8, u8);
    deserialize_key_number!(deserialize_u16, visit_u16, u16);
    deserialize_key_number!(deserialize_u32, visit_u32, u32);
    deserialize_key_number!(deserialize_u64, visit_u64, u64);
    deserialize_key_number!(deserialize_u128, visit_u128, u128);
    deserialize_key_number!(deserialize_f32, visit_f32, f32);
    deserialize_key_number!(deserialize_f64, visit_f64, f64);

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            "true" => visitor.visit_bool(true),
            "false" => visitor.visit_bool(false),
            other => Err(Error::new(format!(
                "mapping key {other:?} is not a valid boolean"
            ))),
        }
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let mut chars = self.value.chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) => visitor.visit_char(ch),
            _ => Err(Error::new(format!(
                "mapping key {:?} is not a single character",
                self.value
            ))),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.value)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.value)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.value)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(EnumValue {
            variant: self.value.to_owned(),
            value: None,
        })
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    serde::forward_to_deserialize_any! {
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct ignored_any
    }
}

struct EnumValue {
    variant: String,
    value: Option<Value>,
}

impl<'de> EnumAccess<'de> for EnumValue {
    type Error = Error;
    type Variant = VariantValue;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let deserializer: de::value::StringDeserializer<Error> = self.variant.into_deserializer();
        let variant = seed.deserialize(deserializer)?;
        Ok((variant, VariantValue { value: self.value }))
    }
}

struct VariantValue {
    value: Option<Value>,
}

impl<'de> VariantAccess<'de> for VariantValue {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        match self.value {
            None => Ok(()),
            Some(_) => Err(Error::new("expected a unit enum variant")),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.value {
            Some(value) => seed.deserialize(value),
            None => Err(Error::new("expected a newtype enum variant")),
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Some(Value::Seq(values)) => visitor.visit_seq(SeqValues {
                iter: values.into_iter(),
            }),
            Some(_) => Err(Error::new("expected a sequence for a tuple enum variant")),
            None => Err(Error::new("expected a tuple enum variant")),
        }
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Some(Value::Map(map)) => visitor.visit_map(MapValues {
                iter: map.into_iter(),
                value: None,
            }),
            Some(_) => Err(Error::new("expected a mapping for a struct enum variant")),
            None => Err(Error::new("expected a struct enum variant")),
        }
    }
}

struct ValueSerializer;

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = SeqSerializer;
    type SerializeTuple = SeqSerializer;
    type SerializeTupleStruct = SeqSerializer;
    type SerializeTupleVariant = TupleVariantSerializer;
    type SerializeMap = MapSerializer;
    type SerializeStruct = MapSerializer;
    type SerializeStructVariant = StructVariantSerializer;

    fn serialize_bool(self, value: bool) -> Result<Value, Error> {
        Ok(Value::Bool(value))
    }

    fn serialize_i8(self, value: i8) -> Result<Value, Error> {
        Ok(Value::Int(i64::from(value)))
    }

    fn serialize_i16(self, value: i16) -> Result<Value, Error> {
        Ok(Value::Int(i64::from(value)))
    }

    fn serialize_i32(self, value: i32) -> Result<Value, Error> {
        Ok(Value::Int(i64::from(value)))
    }

    fn serialize_i64(self, value: i64) -> Result<Value, Error> {
        Ok(Value::Int(value))
    }

    fn serialize_i128(self, value: i128) -> Result<Value, Error> {
        i64::try_from(value)
            .map(Value::Int)
            .map_err(|_| Error::new(format!("integer {value} does not fit in handlist")))
    }

    fn serialize_u8(self, value: u8) -> Result<Value, Error> {
        Ok(Value::Int(i64::from(value)))
    }

    fn serialize_u16(self, value: u16) -> Result<Value, Error> {
        Ok(Value::Int(i64::from(value)))
    }

    fn serialize_u32(self, value: u32) -> Result<Value, Error> {
        Ok(Value::Int(i64::from(value)))
    }

    fn serialize_u64(self, value: u64) -> Result<Value, Error> {
        i64::try_from(value)
            .map(Value::Int)
            .map_err(|_| Error::new(format!("integer {value} does not fit in handlist")))
    }

    fn serialize_u128(self, value: u128) -> Result<Value, Error> {
        i64::try_from(value)
            .map(Value::Int)
            .map_err(|_| Error::new(format!("integer {value} does not fit in handlist")))
    }

    fn serialize_f32(self, value: f32) -> Result<Value, Error> {
        Ok(Value::Float(f64::from(value)))
    }

    fn serialize_f64(self, value: f64) -> Result<Value, Error> {
        Ok(Value::Float(value))
    }

    fn serialize_char(self, value: char) -> Result<Value, Error> {
        Ok(Value::String(value.to_string()))
    }

    fn serialize_str(self, value: &str) -> Result<Value, Error> {
        Ok(Value::String(value.to_owned()))
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Value, Error> {
        Err(Error::new("binary data is not supported by handlist"))
    }

    fn serialize_none(self) -> Result<Value, Error> {
        Ok(Value::Null)
    }

    fn serialize_some<T>(self, value: &T) -> Result<Value, Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value, Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value, Error> {
        Ok(Value::String(variant.to_owned()))
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Value, Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, Error>
    where
        T: ?Sized + Serialize,
    {
        let mut map = Map::new();
        map.insert(variant.to_owned(), value.serialize(ValueSerializer)?);
        Ok(Value::Map(map))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<SeqSerializer, Error> {
        Ok(SeqSerializer {
            items: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<SeqSerializer, Error> {
        Ok(SeqSerializer {
            items: Vec::with_capacity(len),
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<SeqSerializer, Error> {
        Ok(SeqSerializer {
            items: Vec::with_capacity(len),
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<TupleVariantSerializer, Error> {
        Ok(TupleVariantSerializer {
            variant: variant.to_owned(),
            items: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<MapSerializer, Error> {
        Ok(MapSerializer {
            entries: Vec::with_capacity(len.unwrap_or(0)),
            pending_key: None,
        })
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<MapSerializer, Error> {
        Ok(MapSerializer {
            entries: Vec::with_capacity(len),
            pending_key: None,
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<StructVariantSerializer, Error> {
        Ok(StructVariantSerializer {
            variant: variant.to_owned(),
            entries: Vec::with_capacity(len),
        })
    }
}

struct KeySerializer;

impl Serializer for KeySerializer {
    type Ok = String;
    type Error = Error;
    type SerializeSeq = Impossible<String, Error>;
    type SerializeTuple = Impossible<String, Error>;
    type SerializeTupleStruct = Impossible<String, Error>;
    type SerializeTupleVariant = Impossible<String, Error>;
    type SerializeMap = Impossible<String, Error>;
    type SerializeStruct = Impossible<String, Error>;
    type SerializeStructVariant = Impossible<String, Error>;

    fn serialize_bool(self, value: bool) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_i8(self, value: i8) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_i16(self, value: i16) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_i32(self, value: i32) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_i64(self, value: i64) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_i128(self, value: i128) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_u8(self, value: u8) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_u16(self, value: u16) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_u32(self, value: u32) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_u64(self, value: u64) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_u128(self, value: u128) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_f32(self, _value: f32) -> Result<String, Error> {
        Err(Error::new(
            "float mapping keys are not supported by handlist",
        ))
    }

    fn serialize_f64(self, _value: f64) -> Result<String, Error> {
        Err(Error::new(
            "float mapping keys are not supported by handlist",
        ))
    }

    fn serialize_char(self, value: char) -> Result<String, Error> {
        Ok(value.to_string())
    }

    fn serialize_str(self, value: &str) -> Result<String, Error> {
        Ok(value.to_owned())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<String, Error> {
        Err(Error::new(
            "binary mapping keys are not supported by handlist",
        ))
    }

    fn serialize_none(self) -> Result<String, Error> {
        Err(Error::new("handlist mapping keys must be strings"))
    }

    fn serialize_some<T>(self, value: &T) -> Result<String, Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<String, Error> {
        Err(Error::new("handlist mapping keys must be strings"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<String, Error> {
        Err(Error::new("handlist mapping keys must be strings"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<String, Error> {
        Err(Error::new(
            "enum mapping keys are not supported by handlist",
        ))
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<String, Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<String, Error>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::new(
            "enum mapping keys are not supported by handlist",
        ))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Err(Error::new("handlist mapping keys must be strings"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        Err(Error::new("handlist mapping keys must be strings"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(Error::new("handlist mapping keys must be strings"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(Error::new("handlist mapping keys must be strings"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(Error::new("handlist mapping keys must be strings"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Err(Error::new("handlist mapping keys must be strings"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(Error::new("handlist mapping keys must be strings"))
    }

    fn collect_str<T>(self, value: &T) -> Result<String, Error>
    where
        T: ?Sized + fmt::Display,
    {
        Ok(value.to_string())
    }
}

struct SeqSerializer {
    items: Vec<Value>,
}

impl SerializeSeq for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Seq(self.items))
    }
}

impl SerializeTuple for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Seq(self.items))
    }
}

impl SerializeTupleStruct for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Seq(self.items))
    }
}

struct MapSerializer {
    entries: Vec<(String, Value)>,
    pending_key: Option<String>,
}

impl MapSerializer {
    fn push_entry(&mut self, key: String, value: Value) -> Result<(), Error> {
        if self.entries.iter().any(|(existing, _)| existing == &key) {
            return Err(Error::new(format!("duplicate mapping key {key:?}")));
        }
        self.entries.push((key, value));
        Ok(())
    }
}

impl SerializeMap for MapSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        let key = key.serialize(KeySerializer)?;
        self.pending_key = Some(key);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        let Some(key) = self.pending_key.take() else {
            return Err(Error::new("mapping value serialized before its key"));
        };
        let value = value.serialize(ValueSerializer)?;
        self.push_entry(key, value)
    }

    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<(), Error>
    where
        K: ?Sized + Serialize,
        V: ?Sized + Serialize,
    {
        let key = key.serialize(KeySerializer)?;
        let value = value.serialize(ValueSerializer)?;
        self.push_entry(key, value)
    }

    fn end(self) -> Result<Value, Error> {
        if self.pending_key.is_some() {
            return Err(Error::new("mapping key serialized without a value"));
        }
        Ok(Value::Map(Map::from_iter(self.entries)))
    }
}

impl SerializeStruct for MapSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        let value = value.serialize(ValueSerializer)?;
        self.push_entry(key.to_owned(), value)
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Map(Map::from_iter(self.entries)))
    }
}

struct TupleVariantSerializer {
    variant: String,
    items: Vec<Value>,
}

impl SerializeTupleVariant for TupleVariantSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        let mut map = Map::new();
        map.insert(self.variant, Value::Seq(self.items));
        Ok(Value::Map(map))
    }
}

struct StructVariantSerializer {
    variant: String,
    entries: Vec<(String, Value)>,
}

impl SerializeStructVariant for StructVariantSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        if self
            .entries
            .iter()
            .any(|(existing, _)| existing.as_str() == key)
        {
            return Err(Error::new(format!("duplicate mapping key {key:?}")));
        }
        self.entries
            .push((key.to_owned(), value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        let mut inner = Map::new();
        for (key, value) in self.entries {
            inner.insert(key, value);
        }
        let mut map = Map::new();
        map.insert(self.variant, Value::Map(inner));
        Ok(Value::Map(map))
    }
}
