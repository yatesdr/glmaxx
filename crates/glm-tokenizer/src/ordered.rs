use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{MapAccess, SeqAccess, Visitor},
    ser::{SerializeMap, SerializeSeq},
};

/// JSON value that retains the client's object-key order. The pinned GLM-5.2
/// chat template iterates tool-schema and argument mappings, so converting
/// through `serde_json::Value` would silently reorder the prompt.
#[derive(Clone, Debug, PartialEq)]
pub enum OrderedValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
}

impl OrderedValue {
    #[must_use]
    pub fn object(&self) -> Option<&[(String, Self)]> {
        match self {
            Self::Object(entries) => Some(entries),
            _ => None,
        }
    }

    #[must_use]
    pub fn string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Self> {
        self.object()?
            .iter()
            .find_map(|(candidate, value)| (candidate == key).then_some(value))
    }

    #[must_use]
    pub fn contains_nul(&self) -> bool {
        match self {
            Self::String(value) => value.contains('\0'),
            Self::Array(values) => values.iter().any(Self::contains_nul),
            Self::Object(entries) => entries
                .iter()
                .any(|(key, value)| key.contains('\0') || value.contains_nul()),
            Self::Null | Self::Bool(_) | Self::Number(_) => false,
        }
    }

    pub(crate) fn python_json(&self) -> Result<String, serde_json::Error> {
        let mut output = String::new();
        self.write_python_json(&mut output)?;
        Ok(output)
    }

    fn write_python_json(&self, output: &mut String) -> Result<(), serde_json::Error> {
        match self {
            Self::Null => output.push_str("null"),
            Self::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
            Self::Number(value) => output.push_str(&value.to_string()),
            Self::String(value) => output.push_str(&serde_json::to_string(value)?),
            Self::Array(values) => {
                output.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    value.write_python_json(output)?;
                }
                output.push(']');
            }
            Self::Object(entries) => {
                output.push('{');
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    output.push_str(&serde_json::to_string(key)?);
                    output.push_str(": ");
                    value.write_python_json(output)?;
                }
                output.push('}');
            }
        }
        Ok(())
    }
}

impl Serialize for OrderedValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Null => serializer.serialize_none(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Number(value) => value.serialize(serializer),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(values) => {
                let mut sequence = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    sequence.serialize_element(value)?;
                }
                sequence.end()
            }
            Self::Object(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for OrderedValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(OrderedValueVisitor)
    }
}

struct OrderedValueVisitor;

impl<'de> Visitor<'de> for OrderedValueVisitor {
    type Value = OrderedValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(OrderedValue::Null)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(OrderedValue::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(OrderedValue::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(OrderedValue::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(OrderedValue::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(OrderedValue::Number)
            .ok_or_else(|| E::custom("JSON number must be finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(OrderedValue::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(OrderedValue::String(value))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0));
        while let Some(value) = sequence.next_element()? {
            values.push(value);
        }
        Ok(OrderedValue::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut entries = Vec::with_capacity(map.size_hint().unwrap_or(0));
        while let Some((key, value)) = map.next_entry()? {
            if entries
                .iter()
                .any(|(existing, _): &(String, OrderedValue)| existing == &key)
            {
                return Err(serde::de::Error::custom("duplicate JSON object key"));
            }
            entries.push((key, value));
        }
        Ok(OrderedValue::Object(entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_order_and_python_json_spacing_are_stable() {
        let value: OrderedValue =
            serde_json::from_str(r#"{"z":1,"a":{"x":"北京","y":[true,null]}}"#).unwrap();
        assert_eq!(
            value.python_json().unwrap(),
            r#"{"z": 1, "a": {"x": "北京", "y": [true, null]}}"#
        );
        let keys: Vec<_> = value
            .object()
            .unwrap()
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        assert_eq!(keys, ["z", "a"]);
        assert!(serde_json::from_str::<OrderedValue>(r#"{"x":1,"x":2}"#).is_err());
    }
}
