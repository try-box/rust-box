use alloc::vec::Drain;

#[derive(Clone)]
pub struct PriorityQueue<P, V> {
    data: Vec<(P, V)>,
}

impl<P: Ord, V> Default for PriorityQueue<P, V> {
    fn default() -> PriorityQueue<P, V> {
        PriorityQueue::new()
    }
}

impl<P, V> From<Vec<(P, V)>> for PriorityQueue<P, V> {
    #[inline]
    fn from(data: Vec<(P, V)>) -> Self {
        PriorityQueue { data }
    }
}

impl<P: Ord, V> PriorityQueue<P, V> {
    #[inline]
    pub fn new() -> Self {
        PriorityQueue { data: Vec::new() }
    }

    #[inline]
    pub fn push(&mut self, key: P, value: V) {
        self.data.push((key, value));
        self.heapify_up(self.data.len() - 1);
    }

    #[inline]
    pub fn peek(&self) -> Option<&(P, V)> {
        self.data.first()
    }

    #[inline]
    pub fn pop(&mut self) -> Option<(P, V)> {
        if self.data.is_empty() {
            return None;
        }

        let len = self.data.len();
        self.data.swap(0, len - 1);
        let result = self.data.pop();
        self.heapify_down(0);
        result
    }

    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, (P, V)> {
        self.data.iter()
    }

    #[inline]
    pub fn drain<R>(&mut self) -> Drain<'_, (P, V)> {
        self.data.drain(..)
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit()
    }

    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.data.shrink_to(min_capacity)
    }

    #[inline]
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&(P, V)) -> bool,
    {
        self.data.retain(f);
    }

    #[inline]
    pub fn clear(&mut self) {
        self.data.clear();
    }

    #[inline]
    pub fn append(&mut self, other: &mut Self) {
        for (k, v) in other.data.drain(..) {
            self.push(k, v);
        }
    }

    #[inline]
    pub fn extend<I: IntoIterator<Item = (P, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.push(k, v);
        }
    }

    #[inline]
    pub fn to_vec(&self) -> Vec<(P, V)>
    where
        P: Clone,
        V: Clone,
    {
        self.data.to_vec()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    #[inline]
    fn heapify_up(&mut self, mut index: usize) {
        while index > 0 {
            let parent_index = (index - 1) / 2;
            if self.data[index].0 > self.data[parent_index].0 {
                self.data.swap(index, parent_index);
                index = parent_index;
            } else {
                break;
            }
        }
    }

    #[inline]
    fn heapify_down(&mut self, mut index: usize) {
        let len = self.data.len();
        while index < len {
            let left_child_index = 2 * index + 1;
            let right_child_index = 2 * index + 2;
            let mut largest = index;

            if left_child_index < len && self.data[left_child_index].0 > self.data[largest].0 {
                largest = left_child_index;
            }

            if right_child_index < len && self.data[right_child_index].0 > self.data[largest].0 {
                largest = right_child_index;
            }

            if largest != index {
                self.data.swap(index, largest);
                index = largest;
            } else {
                break;
            }
        }
    }
}

impl<P, V> IntoIterator for PriorityQueue<P, V> {
    type Item = (P, V);
    type IntoIter = alloc::vec::IntoIter<(P, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
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
        serializer.collect_seq(self.data.iter())
    }
}

#[cfg(feature = "serde")]
impl<'de, P, V> serde::de::Deserialize<'de> for PriorityQueue<P, V>
where
    P: serde::de::Deserialize<'de> + Ord,
    V: serde::de::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct PriorityQueueVisitor<P, V>(core::marker::PhantomData<(P, V)>);

        impl<'de, P, V> serde::de::Visitor<'de> for PriorityQueueVisitor<P, V>
        where
            P: serde::de::Deserialize<'de> + Ord,
            V: serde::de::Deserialize<'de>,
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
fn test_priorityqueue_serde() {
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
