use core::ops::{Deref, DerefMut};

#[derive(Clone)]
pub struct BinaryHeap<T>(alloc::collections::BinaryHeap<T>);

impl<T> Deref for BinaryHeap<T> {
    type Target = alloc::collections::BinaryHeap<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for BinaryHeap<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Ord> Default for BinaryHeap<T> {
    #[inline]
    fn default() -> BinaryHeap<T> {
        BinaryHeap::new()
    }
}

impl<T: Ord> From<Vec<T>> for BinaryHeap<T> {
    #[inline]
    fn from(data: Vec<T>) -> Self {
        BinaryHeap(alloc::collections::BinaryHeap::from(data))
    }
}

impl<T: Ord> BinaryHeap<T> {
    #[inline]
    pub fn new() -> Self {
        BinaryHeap(alloc::collections::BinaryHeap::new())
    }

    #[inline]
    pub fn iter_sorted(&self) -> alloc::vec::IntoIter<&T> {
        let mut refs: Vec<_> = self.0.iter().collect();
        refs.sort_by(|a, b| b.cmp(a));
        refs.into_iter()
    }

    #[inline]
    pub fn drain_sorted(&mut self) -> impl Iterator<Item = T> {
        let mut items = self.0.drain().collect::<Vec<_>>();
        items.sort_by(|a, b| b.cmp(a));
        items.into_iter()
    }

    #[inline]
    pub fn into_sorted_vec(self) -> Vec<T> {
        self.0.into_sorted_vec()
    }
}

#[cfg(feature = "serde")]
impl<V> serde::ser::Serialize for BinaryHeap<V>
where
    V: serde::ser::Serialize + Ord,
{
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: serde::ser::Serializer,
    {
        serializer.collect_seq(self.0.iter())
    }
}

#[cfg(feature = "serde")]
impl<'de, V> serde::de::Deserialize<'de> for BinaryHeap<V>
where
    V: serde::de::Deserialize<'de> + Ord,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct BinaryHeapVisitor<V>(core::marker::PhantomData<V>);

        impl<'de, V> serde::de::Visitor<'de> for BinaryHeapVisitor<V>
        where
            V: serde::de::Deserialize<'de> + Ord,
        {
            type Value = BinaryHeap<V>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(formatter, "a array")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut values = BinaryHeap::new();
                while let Some(value) = seq.next_element()? {
                    values.push(value);
                }
                Ok(values)
            }
        }

        deserializer.deserialize_seq(BinaryHeapVisitor(core::marker::PhantomData))
    }
}

#[test]
fn test_serde() {
    use alloc::vec::Vec;

    fn into_vec(map: &mut BinaryHeap<u8>) -> Vec<u8> {
        let mut vec = Vec::new();
        while let Some(item) = map.pop() {
            vec.push(item);
        }
        vec
    }

    let mut map: BinaryHeap<u8> = BinaryHeap::new();
    map.push(2);
    map.push(1);
    map.push(9);
    map.push(3);
    map.push(5);

    let data = bincode::serialize(&map).unwrap();
    let mut map: BinaryHeap<u8> = bincode::deserialize(&data).unwrap();
    assert_eq!(into_vec(&mut map), [9, 5, 3, 2, 1]);
}

#[test]
fn test_iter_sorted() {
    let mut map: BinaryHeap<u8> = BinaryHeap::new();
    map.push(2);
    map.push(1);
    map.push(9);
    map.push(3);
    map.push(5);

    let data = map.iter_sorted().collect::<Vec<_>>();
    assert_eq!(data, [&9, &5, &3, &2, &1]);
}

#[test]
fn test_drain_sorted() {
    let mut map: BinaryHeap<u8> = BinaryHeap::new();
    map.push(2);
    map.push(1);
    map.push(9);
    map.push(3);
    map.push(5);

    let data = map.drain_sorted().collect::<Vec<_>>();
    assert_eq!(data, [9, 5, 3, 2, 1]);
}

#[test]
fn test_into_sorted_vec() {
    let mut map: BinaryHeap<u8> = BinaryHeap::new();
    map.push(2);
    map.push(1);
    map.push(9);
    map.push(3);
    map.push(5);

    let data = map.into_sorted_vec();
    assert_eq!(data, [1, 2, 3, 5, 9]);
}
