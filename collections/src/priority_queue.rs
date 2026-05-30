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

#[cfg(feature = "serde")]
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

    let data = postcard::to_stdvec(&map).unwrap();
    let mut map: PriorityQueue<u8, i16> = postcard::from_bytes(&data).unwrap();
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

#[test]
fn test_priority_queue_empty() {
    let mut heap: PriorityQueue<u8, i16> = PriorityQueue::new();
    assert_eq!(heap.len(), 0);
    assert!(heap.is_empty());
    assert_eq!(heap.peek(), None);
    assert_eq!(heap.pop(), None);
    assert_eq!(
        heap.drain_sorted().collect::<Vec<_>>(),
        Vec::<(u8, i16)>::new()
    );
    assert_eq!(heap.into_sorted_vec(), Vec::<(u8, i16)>::new());
}

#[test]
fn test_priority_queue_single_element() {
    let mut heap: PriorityQueue<u8, i16> = PriorityQueue::new();
    heap.push(5, 55);
    assert_eq!(heap.len(), 1);
    assert!(!heap.is_empty());
    assert_eq!(heap.peek(), Some((&5, &55)));
    assert_eq!(heap.pop(), Some((5, 55)));
    assert!(heap.is_empty());

    // drain_sorted with one element
    let mut heap2: PriorityQueue<u8, i16> = PriorityQueue::new();
    heap2.push(7, 77);
    let drained: Vec<(u8, i16)> = heap2.drain_sorted().collect();
    assert_eq!(drained, [(7, 77)]);

    // into_sorted_vec with one element
    let mut heap3: PriorityQueue<u8, i16> = PriorityQueue::new();
    heap3.push(3, 33);
    assert_eq!(heap3.into_sorted_vec(), [(3, 33)]);
}

#[test]
fn test_priority_queue_same_priority() {
    let mut heap: PriorityQueue<u8, i16> = PriorityQueue::new();
    // Same priority - should maintain FIFO order (within same priority, sorted by value which is EqOrdWrapper)
    heap.push(1, 10);
    heap.push(1, 20);
    heap.push(1, 30);
    heap.push(2, 99);
    heap.push(2, 88);
    assert_eq!(heap.len(), 5);

    // drain_sorted: sorted by priority descending, then by value descending (max-heap)
    let drained = heap.drain_sorted().collect::<Vec<_>>();
    assert_eq!(drained, [(2, 99), (2, 88), (1, 30), (1, 20), (1, 10)]);
}

#[test]
fn test_priority_queue_clear() {
    let mut heap: PriorityQueue<u8, i16> = PriorityQueue::new();
    heap.push(3, 33);
    heap.push(1, 11);
    heap.push(4, 44);
    heap.push(5, 55);
    assert_eq!(heap.len(), 4);

    heap.clear();
    assert_eq!(heap.len(), 0);
    assert!(heap.is_empty());
    assert_eq!(heap.peek(), None);
    assert_eq!(heap.pop(), None);
}

#[cfg(feature = "serde")]
#[test]
fn test_priority_queue_serde_empty() {
    let heap: PriorityQueue<u8, i16> = PriorityQueue::new();
    assert!(heap.is_empty());

    let data = postcard::to_stdvec(&heap).unwrap();
    let heap: PriorityQueue<u8, i16> = postcard::from_bytes(&data).unwrap();
    assert!(heap.is_empty());
    assert_eq!(heap.len(), 0);
}
