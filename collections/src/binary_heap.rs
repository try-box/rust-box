#[derive(Clone)]
pub struct BinaryHeap<T> {
    data: Vec<T>,
}

impl<T: Ord> Default for BinaryHeap<T> {
    #[inline]
    fn default() -> BinaryHeap<T> {
        BinaryHeap::new()
    }
}

impl<T> From<Vec<T>> for BinaryHeap<T> {
    #[inline]
    fn from(data: Vec<T>) -> Self {
        BinaryHeap { data }
    }
}

impl<T: Ord> BinaryHeap<T> {
    #[inline]
    pub fn new() -> Self {
        BinaryHeap { data: Vec::new() }
    }

    #[inline]
    pub fn push(&mut self, value: T) {
        self.data.push(value);
        self.heapify_up(self.data.len() - 1);
    }

    #[inline]
    pub fn peek(&self) -> Option<&T> {
        self.data.first()
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
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
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.data.iter()
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
        F: FnMut(&T) -> bool,
    {
        self.data.retain(f);
    }

    #[inline]
    pub fn clear(&mut self) {
        self.data.clear();
    }

    #[inline]
    pub fn append(&mut self, other: &mut Self) {
        for item in other.data.drain(..) {
            self.push(item);
        }
    }

    #[inline]
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.push(item);
        }
    }

    #[inline]
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
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
            if self.data[index] > self.data[parent_index] {
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

            if left_child_index < len && self.data[left_child_index] > self.data[largest] {
                largest = left_child_index;
            }

            if right_child_index < len && self.data[right_child_index] > self.data[largest] {
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

#[cfg(feature = "serde")]
impl<V> serde::ser::Serialize for BinaryHeap<V>
where
    V: serde::ser::Serialize + Ord,
{
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: serde::ser::Serializer,
    {
        serializer.collect_seq(self.data.iter())
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
fn test_dinaryheap_serde() {
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
