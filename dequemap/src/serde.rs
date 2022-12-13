/*
This code shows how to implement the Serialize and Deserialize traits for the DequeMap
type. This allows instances of DequeMap to be serialized using a serialization library
that supports the serde framework, such as bincode. This enables instances of DequeMap to
be converted into a binary representation that can be stored on disk or transmitted over
the network, and then restored at a later time.

The DequeMapVisitor type is a serde Visitor implementation that knows how to deserialize
a DequeMap instance from a stream of data. The Deserialize implementation for DequeMap
uses this visitor to deserialize a DequeMap from a Deserializer. The IntoDeserializer
implementation for DequeMap allows a DequeMap to be used as a MapAccess implementation,
which enables the Deserialize implementation to work with a wider range of Deserializers.

The code also includes a test that demonstrates how to use the bincode crate to serialize
and deserialize a DequeMap instance.

The above content and some comments in the code are written by ChatGPT.
 */
use core::fmt::{self, Formatter};
use core::marker::PhantomData;
use serde::de::value::MapDeserializer;
use serde::de::{Deserialize, Deserializer, Error, IntoDeserializer, MapAccess, Visitor};
use serde::ser::{Serialize, Serializer};

use super::DequeMap;

/// Requires crate feature `"serde"`
impl<K, V> Serialize for DequeMap<K, V>
where
    K: Serialize + Ord,
    V: Serialize,
{
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: Serializer,
    {
        serializer.collect_map(self)
    }
}

struct DequeMapVisitor<K, V>(PhantomData<(K, V)>);

impl<'de, K, V> Visitor<'de> for DequeMapVisitor<K, V>
where
    K: Deserialize<'de> + Ord + Clone,
    V: Deserialize<'de>,
{
    type Value = DequeMap<K, V>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "a map")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = DequeMap::with_capacity(map.size_hint().unwrap_or(0));
        while let Some((key, value)) = map.next_entry()? {
            values.insert(key, value);
        }
        Ok(values)
    }
}

/// Requires crate feature `"serde"`
impl<'de, K, V> Deserialize<'de> for DequeMap<K, V>
where
    K: Deserialize<'de> + Ord + Clone,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(DequeMapVisitor(PhantomData))
    }
}

impl<'de, K, V, E> IntoDeserializer<'de, E> for DequeMap<K, V>
where
    K: IntoDeserializer<'de, E> + Ord,
    V: IntoDeserializer<'de, E>,
    E: Error,
{
    type Deserializer = MapDeserializer<'de, <Self as IntoIterator>::IntoIter, E>;

    fn into_deserializer(self) -> Self::Deserializer {
        MapDeserializer::new(self.into_iter())
    }
}

#[test]
fn test_dequemap_serde() {
    use alloc::vec::Vec;
    let to_vec = |map: &DequeMap<i32, i32>| {
        map.iter()
            .map(|t| (*t.0, *t.1))
            .collect::<Vec<(i32, i32)>>()
    };

    let mut map = DequeMap::new();
    map.push_back(2, 20);
    map.push_back(1, 10);
    map.push_back(9, 90);
    map.push_back(3, 30);
    map.push_back(5, 50);
    assert_eq!(to_vec(&map), [(2, 20), (1, 10), (9, 90), (3, 30), (5, 50)]);

    let data = bincode::serialize(&map).unwrap();
    let map: DequeMap<i32, i32> = bincode::deserialize(&data).unwrap();
    assert_eq!(to_vec(&map), [(2, 20), (1, 10), (9, 90), (3, 30), (5, 50)]);
}
