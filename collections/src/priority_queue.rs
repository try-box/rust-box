use alloc::collections::BinaryHeap;
use core::ops::{Deref, DerefMut};
use std_ext::wrapper::{EqOrdWrapper, OrdExt};

#[derive(Clone)]
pub struct PriorityQueue<P, V> {
    inner: BinaryHeap<(P, EqOrdWrapper<V>)>,
}

impl<P, V> Deref for PriorityQueue<P, V> {
    type Target = BinaryHeap<(P, EqOrdWrapper<V>)>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<P, V> DerefMut for PriorityQueue<P, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<P: Ord, V> Default for PriorityQueue<P, V> {
    fn default() -> PriorityQueue<P, V> {
        PriorityQueue::new()
    }
}

impl<P: Ord, V> From<Vec<(P, V)>> for PriorityQueue<P, V> {
    #[inline]
    fn from(data: Vec<(P, V)>) -> Self {
        PriorityQueue {
            inner: BinaryHeap::from(
                data.into_iter()
                    .map(|(p, v)| (p, v.eq_ord()))
                    .collect::<Vec<_>>(),
            ),
        }
    }
}

impl<P: Ord, V> PriorityQueue<P, V> {
    #[inline]
    pub fn new() -> Self {
        PriorityQueue {
            inner: BinaryHeap::default(),
        }
    }

    #[inline]
    pub fn push(&mut self, key: P, value: V) {
        self.inner.push((key, value.eq_ord()));
    }

    #[inline]
    pub fn pop(&mut self) -> Option<(P, V)> {
        self.inner.pop().map(|(p, v)| (p, v.take()))
    }

    #[inline]
    pub fn peek(&mut self) -> Option<(&P, &V)> {
        self.inner.peek().map(|(p, v)| (p, v.deref()))
    }

    #[inline]
    pub fn iter_sorted(&self) -> impl Iterator<Item = (&P, &V)> {
        let mut refs: Vec<_> = self.inner.iter().map(|(p, v)| (p, v.deref())).collect();
        refs.sort_by(|a, b| b.0.cmp(a.0));
        refs.into_iter()
    }

    #[inline]
    pub fn drain_sorted(&mut self) -> impl Iterator<Item = (P, V)> {
        let mut items = self
            .inner
            .drain()
            .map(|(p, v)| (p, v.take()))
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.0.cmp(&a.0));
        items.into_iter()
    }

    #[inline]
    pub fn into_sorted_vec(self) -> Vec<(P, V)> {
        self.inner
            .into_sorted_vec()
            .into_iter()
            .map(|(p, v)| (p, v.take()))
            .collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&P, &V)> {
        self.inner.iter().map(|(p, v)| (p, v.deref()))
    }
}

#[cfg(feature = "serde")]
impl<P, V> serde::ser::Serialize for PriorityQueue<P, V>
where
    P: serde::ser::Serialize + Ord,
    V: serde::ser::Serialize,
{
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: serde::ser::Serializer,
    {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(feature = "serde")]
impl<'de, P, V> serde::de::Deserialize<'de> for PriorityQueue<P, V>
where
    P: serde::de::Deserialize<'de> + Ord,
    V: serde::de::Deserialize<'de> + Ord,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct PriorityQueueVisitor<P, V>(core::marker::PhantomData<(P, V)>);

        impl<'de, P, V> serde::de::Visitor<'de> for PriorityQueueVisitor<P, V>
        where
            P: serde::de::Deserialize<'de> + Ord,
            V: serde::de::Deserialize<'de> + Ord,
        {
            type Value = PriorityQueue<P, V>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(formatter, "a array")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut values = PriorityQueue::new();
                while let Some(value) = seq.next_element()? {
                    let (p, v) = value;
                    values.push(p, v);
                }
                Ok(values)
            }
        }

        deserializer.deserialize_seq(PriorityQueueVisitor(core::marker::PhantomData))
    }
}

#[test]
fn test_serde() {
    use alloc::vec::Vec;

    fn into_vec(map: &mut PriorityQueue<u8, i16>) -> Vec<(u8, i16)> {
        let mut vec = Vec::new();
        while let Some(item) = map.pop() {
            vec.push(item);
        }
        vec
    }

    let mut map: PriorityQueue<u8, i16> = PriorityQueue::new();
    map.push(2, 22);
    map.push(1, 11);
    map.push(9, 99);
    map.push(3, 33);
    map.push(5, 55);

    let data = bincode::serialize(&map).unwrap();
    let mut map: PriorityQueue<u8, i16> = bincode::deserialize(&data).unwrap();
    assert_eq!(
        into_vec(&mut map),
        [(9, 99), (5, 55), (3, 33), (2, 22), (1, 11)]
    );
}

#[test]
fn test_iter_sorted() {
    let mut map: PriorityQueue<u8, i16> = PriorityQueue::new();
    map.push(2, 22);
    map.push(1, 11);
    map.push(9, 99);
    map.push(3, 33);
    map.push(5, 55);

    let data = map.iter_sorted().collect::<Vec<_>>();
    assert_eq!(
        data,
        [(&9, &99), (&5, &55), (&3, &33), (&2, &22), (&1, &11)]
    );
}

#[test]
fn test_drain_sorted() {
    let mut map: PriorityQueue<u8, i16> = PriorityQueue::new();
    map.push(2, 22);
    map.push(1, 88);
    map.push(9, 10);
    map.push(3, 33);
    map.push(5, 66);

    let data = map.drain_sorted().collect::<Vec<_>>();
    assert_eq!(data, [(9, 10), (5, 66), (3, 33), (2, 22), (1, 88)]);
}

#[test]
fn test_into_sorted_vec() {
    let mut map: PriorityQueue<u8, i16> = PriorityQueue::new();
    map.push(2, 22);
    map.push(1, 88);
    map.push(9, 10);
    map.push(3, 33);
    map.push(5, 66);

    let data = map.into_sorted_vec();
    assert_eq!(data, [(1, 88), (2, 22), (3, 33), (5, 66), (9, 10)]);
}
