use alloc::collections::vec_deque::IntoIter as DequeIntoIter;
use alloc::collections::vec_deque::Iter as DequeIter;

use alloc::collections::VecDeque;
use alloc::collections::{btree_map, BTreeMap};
use core::borrow::Borrow;
use core::fmt;
use core::iter::DoubleEndedIterator;
use core::iter::ExactSizeIterator;
use core::iter::FromIterator;
use core::iter::FusedIterator;
use core::mem::replace;
use core::ops::{Index, IndexMut};

///Double-ended queue with Map feature.
///
/// When the element is present, the maximum time complexity is O(n). So it is not suitable for
/// processing large batches of data with too many duplicates.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, `None` is returned.
    ///
    /// If the map did have this key present, the value is updated, and the old
    /// value is returned. The key is not updated, though; this matters for
    /// types that can be `==` without being identical.
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
    pub fn entry(&mut self, key: K) -> Entry<K, V>
    where
        K: Ord,
    {
        match self.entries.entry(key) {
            btree_map::Entry::Vacant(entry) => Entry::Vacant(VacantEntry {
                vacant: entry,
                indices: &mut self.indices,
            }),
            btree_map::Entry::Occupied(entry) => Entry::Occupied(OccupiedEntry { occupied: entry }),
        }
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
    /// Reserves capacity for at least additional more elements to be inserted in the given VecDeque.
    /// The collection may reserve more space to avoid frequent reallocations.
    pub fn reserve(&mut self, additional: usize) {
        self.indices.reserve(additional);
    }

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
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        self.entries.get(k)
    }

    #[inline]
    pub fn get_key_value<Q: ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        self.entries.get_key_value(key)
    }

    #[inline]
    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        self.entries.get_mut(k)
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            inner: self.indices.iter(),
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
    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        self.entries.contains_key(k)
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
}

impl<'a, K, Q, V> Index<&'a Q> for DequeMap<K, V>
where
    K: Borrow<Q> + Ord,
    Q: Ord,
{
    type Output = V;

    fn index(&self, key: &'a Q) -> &Self::Output {
        self.get(key).expect("no entry found for key")
    }
}

impl<K: Ord, V> Index<usize> for DequeMap<K, V> {
    type Output = V;

    fn index(&self, index: usize) -> &Self::Output {
        let key = self
            .indices
            .get(index)
            .expect("DequeMap: index out of bounds");
        self.entries
            .get(key)
            .expect("DequeMap: index out of bounds")
    }
}

impl<K: Ord, V> IndexMut<usize> for DequeMap<K, V> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let key = self
            .indices
            .get(index)
            .expect("DequeMap: index out of bounds");
        self.entries
            .get_mut(key)
            .expect("DequeMap: index out of bounds")
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
            inner: self.indices.into_iter(),
            entries: self.entries,
        }
    }
}

impl<'a, K, V> Extend<(&'a K, &'a V)> for DequeMap<K, V>
where
    K: Ord + Copy,
    V: Copy,
{
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (&'a K, &'a V)>,
    {
        self.extend(iter.into_iter().map(|(key, value)| (*key, *value)))
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

impl<K, V> FromIterator<(K, V)> for DequeMap<K, V>
where
    K: Ord + Clone,
{
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (K, V)>,
    {
        let mut map = DequeMap::new();
        map.extend(iter);
        map
    }
}

impl<K, V, const N: usize> From<[(K, V); N]> for DequeMap<K, V>
where
    K: Ord + Clone,
{
    fn from(items: [(K, V); N]) -> Self {
        let mut map = DequeMap::new();
        map.extend(items);
        map
    }
}

impl<'a, K: Ord, V> IntoIterator for &'a DequeMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Debug, Clone)]
pub struct Iter<'a, K, V> {
    inner: DequeIter<'a, K>,
    entries: &'a BTreeMap<K, V>,
}

impl<'a, K: Ord, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(k) = self.inner.next() {
            self.entries.get(k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.count()
    }
}

impl<'a, K: Ord, V> DoubleEndedIterator for Iter<'a, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(k) = self.inner.next_back() {
            self.entries.get(k).map(|v| (k, v))
        } else {
            None
        }
    }
}

impl<'a, K: Ord, V> ExactSizeIterator for Iter<'a, K, V> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, K: Ord, V> FusedIterator for Iter<'a, K, V> {}

pub struct IntoIter<K, V> {
    inner: DequeIntoIter<K>,
    entries: BTreeMap<K, V>,
}

impl<K: Ord, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(k) = self.inner.next() {
            self.entries.remove(&k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.count()
    }
}

impl<K: Ord, V> DoubleEndedIterator for IntoIter<K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(k) = self.inner.next_back() {
            self.entries.remove(&k).map(|v| (k, v))
        } else {
            None
        }
    }
}

impl<K: Ord, V> ExactSizeIterator for IntoIter<K, V> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<K: Ord, V> FusedIterator for IntoIter<K, V> {}

/// A view into a single entry in a map, which may either be vacant or occupied.
///
/// This `enum` is constructed from the [`entry`] method on [`DequeMap`].
///
/// [`entry`]: DequeMap::entry
pub enum Entry<'a, K, V> {
    /// A vacant entry.
    Vacant(VacantEntry<'a, K, V>),
    /// An occupied entry.
    Occupied(OccupiedEntry<'a, K, V>),
}

impl<'a, K: Ord, V> Entry<'a, K, V> {
    /// Ensures a value is in the entry by inserting the default if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_insert(self, default: V) -> &'a mut V
    where
        K: Clone,
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result
    /// of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V
    where
        K: Clone,
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Ensures a value is in the entry by inserting,
    /// if empty, the result of the default function.
    ///
    /// This method allows for generating key-derived values for
    /// insertion by providing the default function a reference
    /// to the key that was moved during the `.entry(key)` method call.
    ///
    /// The reference to the moved key is provided
    /// so that cloning or copying the key is
    /// unnecessary, unlike with `.or_insert_with(|| ... )`.
    pub fn or_insert_with_key<F: FnOnce(&K) -> V>(self, default: F) -> &'a mut V
    where
        K: Clone,
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(value)
            }
        }
    }

    /// Returns a reference to this entry’s key.
    pub fn key(&self) -> &K {
        match *self {
            Self::Occupied(ref entry) => entry.key(),
            Self::Vacant(ref entry) => entry.key(),
        }
    }

    /// Provides in-place mutable access to an occupied entry
    /// before any potential inserts into the map.
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        match self {
            Self::Occupied(mut entry) => {
                f(entry.get_mut());
                Self::Occupied(entry)
            }
            Self::Vacant(entry) => Self::Vacant(entry),
        }
    }
}

impl<'a, K, V> Entry<'a, K, V>
where
    K: Ord + Clone,
    V: Default,
{
    /// Ensures a value is in the entry by inserting the default value if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_default(self) -> &'a mut V {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(Default::default()),
        }
    }
}

impl<'a, K, V> fmt::Debug for Entry<'a, K, V>
where
    K: fmt::Debug + Ord,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Entry::Vacant(entry) => entry.fmt(f),
            Entry::Occupied(entry) => entry.fmt(f),
        }
    }
}

/// A view into a vacant entry in an [`DequeMap`]. It is part of the [`Entry`] `enum`.
pub struct VacantEntry<'a, K, V> {
    /// The underlying vacant entry.
    vacant: btree_map::VacantEntry<'a, K, V>,
    /// The vector that stores all slots.
    indices: &'a mut VecDeque<K>,
}

impl<'a, K, V> VacantEntry<'a, K, V>
where
    K: Ord,
{
    /// Gets a reference to the key that would be used when inserting a value through the VacantEntry.
    pub fn key(&self) -> &K {
        self.vacant.key()
    }

    /// Take ownership of the key.
    pub fn into_key(self) -> K {
        self.vacant.into_key()
    }

    /// Sets the value of the entry with the `VacantEntry`’s key,
    /// and returns a mutable reference to it.
    pub fn insert(self, value: V) -> &'a mut V
    where
        K: Clone,
    {
        self.indices.push_back(self.vacant.key().clone());
        self.vacant.insert(value)
    }
}

impl<'a, K, V> fmt::Debug for VacantEntry<'a, K, V>
where
    K: fmt::Debug + Ord,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VacantEntry")
            .field("key", self.key())
            .finish()
    }
}

/// A view into an occupied entry in a [`DequeMap`]. It is part of the [`Entry`] `enum`.
pub struct OccupiedEntry<'a, K, V> {
    /// The underlying occupied entry.
    occupied: btree_map::OccupiedEntry<'a, K, V>,
}

impl<'a, K, V> OccupiedEntry<'a, K, V>
where
    K: Ord,
{
    /// Gets a reference to the key in the entry.
    pub fn key(&self) -> &K {
        self.occupied.key()
    }

    /// Gets a reference to the value in the entry.
    pub fn get(&self) -> &V {
        self.occupied.get()
    }

    /// Gets a mutable reference to the value in the entry.
    ///
    /// If you need a reference to the `OccupiedEntry` that may outlive the
    /// destruction of the `Entry` value, see [`into_mut`].
    ///
    /// [`into_mut`]: OccupiedEntry::into_mut
    pub fn get_mut(&mut self) -> &mut V {
        self.occupied.get_mut()
    }

    /// Converts the entry into a mutable reference to its value.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see [`get_mut`].
    ///
    /// [`get_mut`]: OccupiedEntry::get_mut
    pub fn into_mut(self) -> &'a mut V {
        self.occupied.into_mut()
    }

    /// Sets the value of the entry with the `OccupiedEntry`’s key,
    /// and returns the entry’s old value.
    pub fn insert(&mut self, value: V) -> V
    where
        K: Clone,
    {
        replace(self.occupied.get_mut(), value)
    }
}

impl<'a, K, V> fmt::Debug for OccupiedEntry<'a, K, V>
where
    K: fmt::Debug + Ord,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("key", self.key())
            .field("value", self.get())
            .finish()
    }
}

#[test]
fn test_insert() {
    use alloc::vec::Vec;
    let to_vec = |map: &DequeMap<i32, i32>| {
        map.iter()
            .map(|t| (*t.0, *t.1))
            .collect::<Vec<(i32, i32)>>()
    };

    let mut map = DequeMap::new();
    map.insert(2, 20);
    map.insert(1, 10);
    map.insert(9, 90);
    assert_eq!(to_vec(&map), [(2, 20), (1, 10), (9, 90)]);

    map.insert(7, 70);
    map.insert(1, 100);
    assert_eq!(to_vec(&map), [(2, 20), (1, 100), (9, 90), (7, 70)]);

    assert_eq!(map.entries.len(), map.indices.len());

    assert_eq!(map.pop_front(), Some((2, 20)));
    assert_eq!(map.pop_back(), Some((7, 70)));
    assert_eq!(to_vec(&map), [(1, 100), (9, 90)]);

    map.insert(3, 30);
    map.insert(7, 70);
    map.insert(9, 900);
    map.push_back(1, 10);
    assert_eq!(to_vec(&map), [(9, 900), (3, 30), (7, 70), (1, 10)]);
    assert_eq!(map.entries.len(), map.indices.len());
}

#[test]
fn test_entry() {
    use alloc::vec::Vec;
    let to_vec = |map: &DequeMap<i32, i32>| {
        map.iter()
            .map(|t| (*t.0, *t.1))
            .collect::<Vec<(i32, i32)>>()
    };

    let mut map = DequeMap::new();
    map.entry(2).or_insert(20);
    map.entry(1).or_insert(10);
    map.entry(9).or_insert(90);
    map.entry(3).or_insert(30);
    map.entry(5).or_insert(50);
    assert_eq!(map.get(&1), Some(&10));
    assert_eq!(map.get(&2), Some(&20));
    assert_eq!(map.get(&3), Some(&30));
    assert_eq!(map.get(&5), Some(&50));
    assert_eq!(map.get(&9), Some(&90));

    assert_eq!(to_vec(&map), [(2, 20), (1, 10), (9, 90), (3, 30), (5, 50)]);
    assert_eq!(map.entries.len(), map.indices.len());

    map.entry(3).and_modify(|v| *v = 300);

    assert_eq!(to_vec(&map), [(2, 20), (1, 10), (9, 90), (3, 300), (5, 50)]);
    assert_eq!(map.entries.len(), map.indices.len());

    map.entry(7).or_insert_with(|| 70);
    assert_eq!(
        to_vec(&map),
        [(2, 20), (1, 10), (9, 90), (3, 300), (5, 50), (7, 70)]
    );
    assert_eq!(map.entries.len(), map.indices.len());
}

#[test]
fn test_dequemap() {
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
    assert_eq!(to_vec(&map), [(1, 10), (9, 90), (3, 30)]);
    assert_eq!(map.entries.len(), map.indices.len());

    let mut map1: DequeMap<i32, i32> = DequeMap::new();
    map1.push_back(7, 70);
    map1.push_back(9, 900);
    map.extend(map1);
    assert_eq!(to_vec(&map), [(1, 10), (9, 900), (3, 30), (7, 70)]);
    assert_eq!(map.entries.len(), map.indices.len());

    assert_eq!(map.front(), Some((&1, &10)));
    assert_eq!(map.back(), Some((&7, &70)));

    assert_eq!(to_vec(&map), [(1, 10), (9, 900), (3, 30), (7, 70)]);
    assert_eq!(map.entries.len(), map.indices.len());

    map.remove(&3);
    assert_eq!(to_vec(&map), [(1, 10), (9, 900), (7, 70)]);
    assert_eq!(map.entries.len(), map.indices.len());
}
