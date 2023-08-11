// This code is referred from https://github.com/mre/hyperjson/blob/4821e55765f4428b687fd0459de54ba672ff8df0/src/lib.rs

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyFloat, PyList, PyTuple};

use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{self, Serialize, SerializeMap, SerializeSeq, Serializer};

pub(crate) struct SerializePyObject<'p, 'a> {
    pub(crate) py: Python<'p>,
    pub(crate) obj: &'a PyAny
}

impl<'p, 'a> Serialize for SerializePyObject<'p, 'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        macro_rules! cast {
            ($f:expr) => {
                if let Ok(val) = PyTryFrom::try_from(self.obj) {
                    return $f(val);
                }
            };
        }

        macro_rules! extract {
            ($t:ty) => {
                if let Ok(val) = <$t as FromPyObject>::extract(self.obj) {
                    return val.serialize(serializer);
                }
            };
        }

        cast!(|x: &PyDict| {
            let mut map = serializer.serialize_map(Some(x.len()))?;
            for (key, value) in x {
                if key.is_none() {
                    map.serialize_key("null")?;
                } else if let Ok(key) = key.extract::<bool>() {
                    map.serialize_key(if key { "true" } else { "false" })?;
                } else if let Ok(key) = key.str() {
                    let key = key.to_string();//.map_err(debug_py_err)?;
                    map.serialize_key(&key)?;
                } else {
                    return Err(ser::Error::custom(format_args!(
                        "Dictionary key is not a string: {:?}",
                        key
                    )));
                }
                map.serialize_value(&SerializePyObject {
                    py: self.py,
                    obj: value,
                })?;
            }
            map.end()
        });

        cast!(|x: &PyList| {
            let mut seq = serializer.serialize_seq(Some(x.len()))?;
            for element in x {
                seq.serialize_element(&SerializePyObject {
                    py: self.py,
                    obj: element,
                })?
            }
            seq.end()
        });
        cast!(|x: &PyTuple| {
            let mut seq = serializer.serialize_seq(Some(x.len()))?;
            for element in x {
                seq.serialize_element(&SerializePyObject {
                    py: self.py,
                    obj: element,
                })?
            }
            seq.end()
        });

        extract!(String);
        extract!(bool);

        cast!(|x: &PyFloat| x.value().serialize(serializer));
        extract!(u64);
        extract!(i64);

        if self.obj.is_none() {
            return serializer.serialize_unit();
        }

        match self.obj.repr() {
            Ok(repr) => Err(ser::Error::custom(format_args!(
                "Value is not serializable: {}",
                repr,
            ))),
            Err(_) => Err(ser::Error::custom(format_args!(
                "Type is not serializable: {}",
                self.obj.get_type().name().unwrap(),
            ))),
        }
    }
}

#[derive(Copy, Clone)]
pub(crate) struct HyperJsonValue<'a> {
    py: Python<'a>,
    parse_float: &'a Option<PyObject>,
    parse_int: &'a Option<PyObject>,
}

impl<'a> HyperJsonValue<'a> {
    pub fn new(
        py: Python<'a>,
        parse_float: &'a Option<PyObject>,
        parse_int: &'a Option<PyObject>,
    ) -> HyperJsonValue<'a> {
        // We cannot borrow the runtime here,
        // because it wouldn't live long enough
        // let gil = Python::acquire_gil();
        // let py = gil.python();
        HyperJsonValue {
            py,
            parse_float,
            parse_int,
        }
    }
}

impl<'de, 'a> DeserializeSeed<'de> for HyperJsonValue<'a> {
    type Value = PyObject;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'a> HyperJsonValue<'a> {
    fn parse_primitive<E, T>(self, value: T, parser: &PyObject) -> Result<PyObject, E>
        where
            E: de::Error,
            T: ToString,
    {
        match parser.call1(self.py, (value.to_string(),)) {
            Ok(primitive) => Ok(primitive),
            Err(err) => Err(de::Error::custom(err.to_string())),
        }
    }
}

impl<'de, 'a> Visitor<'de> for HyperJsonValue<'a> {
    type Value = PyObject;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("any valid JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
        where
            E: de::Error,
    {
        Ok(value.to_object(self.py))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
    {
        match self.parse_int {
            Some(parser) => self.parse_primitive(value, parser),
            None => Ok(value.to_object(self.py)),
        }
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
    {
        match self.parse_int {
            Some(parser) => self.parse_primitive(value, parser),
            None => Ok(value.to_object(self.py)),
        }
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
    {
        match self.parse_float {
            Some(parser) => self.parse_primitive(value, parser),
            None => Ok(value.to_object(self.py)),
        }
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
    {
        Ok(value.to_object(self.py))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(self.py.None())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
    {
        let mut elements = Vec::new();

        while let Some(elem) = seq.next_element_seed(self)? {
            elements.push(elem);
        }

        Ok(elements.to_object(self.py))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
    {
        let mut entries = BTreeMap::new();

        while let Some((key, value)) = map.next_entry_seed(PhantomData::<String>, self)? {
            entries.insert(key, value);
        }

        Ok(entries.to_object(self.py))
    }
}
