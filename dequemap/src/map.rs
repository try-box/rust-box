use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;

///Double-ended queue with Map feature.
///
/// When the element is present, the maximum time complexity is O(n). So it is not suitable for
/// processing large batches of data with too many duplicates.

#[derive(Clone, Debug)]
pub struct DequeMap<K, V> {
    entries: BTreeMap<K, V>,
    indices: VecDeque<K>,
}

impl<K, V> DequeMap<K, V> {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            indices: VecDeque::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: BTreeMap::default(),
            indices: VecDeque::with_capacity(capacity),
        }
    }
}

impl<K, V> Default for DequeMap<K, V> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::default(),
            indices: VecDeque::default(),
        }
    }
}

impl<K, V> DequeMap<K, V>
where
    K: Clone + Ord,
{
    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(v) = self.entries.get_mut(&key) {
            Some(replace(v, value))
        } else {
            self.entries.insert(key.clone(), value);
            self.indices.push_back(key);
            None
        }
    }

    #[inline]
    pub fn push_back(&mut self, key: K, value: V) -> Option<V> {
        let old_val = self.remove_entry(&key);
        self.entries.insert(key.clone(), value);
        self.indices.push_back(key);
        old_val
    }

    #[inline]
    pub fn push_front(&mut self, key: K, value: V) -> Option<V> {
        let old_val = self.remove_entry(&key);
        self.entries.insert(key.clone(), value);
        self.indices.push_front(key);
        old_val
    }

    #[inline]
    fn remove_entry(&mut self, key: &K) -> Option<V> {
        if let Some(old_val) = self.entries.remove(key) {
            self.remove_from_index(key);
            Some(old_val)
        } else {
            None
        }
    }
}

impl<K, V> DequeMap<K, V> {
    #[inline]
    pub fn clear(&mut self) {
        self.indices.clear();
        self.entries.clear();
    }

    #[inline]
    pub fn remove(&mut self, k: &K) -> Option<V>
    where
        K: Ord,
    {
        if let Some(old_val) = self.entries.remove(k) {
            self.remove_from_index(k);
            Some(old_val)
        } else {
            None
        }
    }

    #[inline]
    pub fn get(&self, k: &K) -> Option<&V>
    where
        K: Ord,
    {
        self.entries.get(k)
    }

    #[inline]
    pub fn contains_key(&self, k: &K) -> bool
    where
        K: Ord,
    {
        self.entries.contains_key(k)
    }

    #[inline]
    pub fn get_mut(&mut self, k: &K) -> Option<&mut V>
    where
        K: Ord,
    {
        self.entries.get_mut(k)
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            idx_iter: self.indices.iter(),
            entries: &self.entries,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    #[inline]
    pub fn front(&self) -> Option<(&K, &V)>
    where
        K: Ord,
    {
        if self.is_empty() {
            return None;
        }
        if let Some(k) = self.indices.front() {
            self.entries.get(k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    pub fn pop_front(&mut self) -> Option<(K, V)>
    where
        K: Ord,
    {
        if let Some(k) = self.indices.pop_front() {
            self.entries.remove(&k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    pub fn back(&self) -> Option<(&K, &V)>
    where
        K: Ord,
    {
        if self.is_empty() {
            return None;
        }
        if let Some(k) = self.indices.back() {
            self.entries.get(k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    pub fn pop_back(&mut self) -> Option<(K, V)>
    where
        K: Ord,
    {
        if let Some(k) = self.indices.pop_back() {
            self.entries.remove(&k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    fn get_index(&self, k: &K) -> Option<usize>
    where
        K: Ord,
    {
        self.indices
            .iter()
            .enumerate()
            .find(|(_, x)| *x == k)
            .map(|(idx, _)| idx)
    }

    #[inline]
    fn remove_from_index(&mut self, k: &K) -> Option<K>
    where
        K: Ord,
    {
        if let Some(idx) = self.get_index(k) {
            self.indices.remove(idx)
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn _check_len(&self) -> bool {
        self.indices.len() == self.entries.len()
    }
}

impl<K, V> IntoIterator for DequeMap<K, V>
where
    K: Ord,
{
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            idx_iter: self.indices.into_iter(),
            entries: self.entries,
        }
    }
}

impl<K, V> Extend<(K, V)> for DequeMap<K, V>
where
    K: Ord + Clone,
{
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

use std::collections::vec_deque::Iter as IdxIter;

pub struct Iter<'a, K: 'a, V: 'a> {
    idx_iter: IdxIter<'a, K>,
    entries: &'a BTreeMap<K, V>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    K: Ord,
{
    type Item = (&'a K, &'a V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(k) = self.idx_iter.next() {
            self.entries.get(k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.idx_iter.size_hint()
    }
}

use std::collections::vec_deque::IntoIter as IdxIntoIter;
use std::mem::replace;

pub struct IntoIter<K, V> {
    idx_iter: IdxIntoIter<K>,
    entries: BTreeMap<K, V>,
}

impl<K, V> Iterator for IntoIter<K, V>
where
    K: Ord,
{
    type Item = (K, V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(k) = self.idx_iter.next() {
            self.entries.remove(&k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.idx_iter.size_hint()
    }
}

#[test]
fn test_insert() {
    let mut map = DequeMap::new();
    map.insert(2, 20);
    map.insert(1, 10);
    map.insert(9, 90);
    map.insert(7, 70);
    map.insert(1, 100);

    println!("--test_insert----, map.entries: {:?}", map.entries);
    println!("map.entries: {:?}", map.entries);
    println!("map.indices: {:?}", map.indices);
    for (k, v) in map.iter() {
        println!("{} = {}", k, v);
    }

    let items: Vec<(i32, i32)> = map.iter().map(|t| (*t.0, *t.1)).collect();
    println!("items: {:?}", items);
    assert_eq!(items, [(2, 20), (1, 100), (9, 90), (7, 70)]);

    assert_eq!(map.pop_front(), Some((2, 20)));
    assert_eq!(map.pop_back(), Some((7, 70)));
    println!("--test_insert----");
    for (k, v) in map.iter() {
        println!("{} = {}", k, v);
    }
    map.insert(3, 30);
    map.insert(7, 70);
    map.insert(9, 900);
    map.push_back(1, 10);

    println!("--test_insert----");
    for (k, v) in map.iter() {
        println!("{} = {}", k, v);
    }
    let items: Vec<(i32, i32)> = map.iter().map(|t| (*t.0, *t.1)).collect();
    println!("items: {:?}", items);
    assert_eq!(items, [(9, 900), (3, 30), (7, 70), (1, 10)]);
}

#[test]
fn test_dequemap() {
    let mut map = DequeMap::new();
    map.push_back(2, 20);
    map.push_back(1, 10);
    map.push_back(9, 90);
    map.push_back(3, 30);
    map.push_back(5, 50);
    assert_eq!(map.get(&1), Some(&10));
    assert_eq!(map.get(&2), Some(&20));
    assert_eq!(map.get(&3), Some(&30));
    assert_eq!(map.get(&5), Some(&50));
    assert_eq!(map.get(&9), Some(&90));
    assert_eq!(map.len(), 5);
    assert_eq!(map.pop_front(), Some((2, 20)));
    assert_eq!(map.len(), 4);
    assert_eq!(map.pop_back(), Some((5, 50)));
    assert_eq!(map.len(), 3);
    println!("--test_dequemap({})--------", map.len());
    for (k, v) in map.iter() {
        println!("{} = {}", k, v);
    }

    let mut map1: DequeMap<i32, i32> = DequeMap::new();
    map1.push_back(7, 70);
    map1.push_back(9, 900);
    map.extend(map1);

    println!("--test_dequemap({})--------", map.len());
    for (i, (k, v)) in map.iter().enumerate() {
        println!("{} {} = {}", i, k, v);
    }

    assert_eq!(map.front(), Some((&1, &10)));
    assert_eq!(map.back(), Some((&7, &70)));

    let items: Vec<(i32, i32)> = map.iter().map(|t| (*t.0, *t.1)).collect();
    println!("items: {:?}", items);
    assert_eq!(items, [(1, 10), (9, 900), (3, 30), (7, 70)]);

    map.remove(&7);
    let items: Vec<(i32, i32)> = map.iter().map(|t| (*t.0, *t.1)).collect();
    println!("items: {:?}", items);
    assert_eq!(items, [(1, 10), (9, 900), (3, 30)]);
}
