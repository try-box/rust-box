use std::borrow::Borrow;
use std::cmp::Ord;
use std::collections::btree_map;
use std::collections::hash_map;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{BuildHasher, Hash};
use std::time::{Duration, Instant};

pub trait CacheMapExt<K, V> {
    fn get_with_timeout<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q> + Ord,
        Q: Hash + Eq + Ord + ?Sized;

    fn get_with_timeout_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q> + Ord,
        Q: Hash + Eq + Ord + ?Sized;

    fn insert_with_timeout(&mut self, k: K, v: V, timeout: Option<Duration>) -> Option<V>;

    fn remove_expired_values(&mut self);
}

pub trait EntryExt<'a, K, V> {
    fn or_insert_with_timeout(self, default: V, timeout: Option<Duration>) -> &'a mut V;
    fn or_insert_with_timeout_f<F: FnOnce() -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V;
    fn or_insert_with_timeout_key_f<F: FnOnce(&K) -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V;
    fn and_modify_with_timeout<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V);
}

impl<K, V, S> CacheMapExt<K, V> for HashMap<K, TimedValue<V>, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn get_with_timeout<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value())
            }
        })
    }

    fn get_with_timeout_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get_mut(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value_mut())
            }
        })
    }

    fn insert_with_timeout(&mut self, k: K, v: V, timeout: Option<Duration>) -> Option<V> {
        self.insert(k, TimedValue::new(v, timeout))
            .map(|tv| tv.into_value())
    }

    fn remove_expired_values(&mut self) {
        self.retain(|_, tv| !tv.is_expired());
    }
}

impl<'a, K, V> EntryExt<'a, K, V> for hash_map::Entry<'a, K, TimedValue<V>>
where
    K: Eq + Hash,
{
    fn or_insert_with_timeout(self, default: V, timeout: Option<Duration>) -> &'a mut V {
        match self {
            hash_map::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default, timeout);
                }
                v.value_mut()
            }
            hash_map::Entry::Vacant(entry) => {
                entry.insert(TimedValue::new(default, timeout)).value_mut()
            }
        }
    }

    fn or_insert_with_timeout_f<F: FnOnce() -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            hash_map::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default(), timeout);
                }
                v.value_mut()
            }
            hash_map::Entry::Vacant(entry) => entry
                .insert(TimedValue::new(default(), timeout))
                .value_mut(),
        }
    }

    fn or_insert_with_timeout_key_f<F: FnOnce(&K) -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            hash_map::Entry::Occupied(entry) => {
                let value = if entry.get().is_expired() {
                    Some(default(entry.key()))
                } else {
                    None
                };
                let v = entry.into_mut();
                if let Some(value) = value {
                    *v = TimedValue::new(value, timeout);
                }
                v.value_mut()
            }
            hash_map::Entry::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(TimedValue::new(value, timeout)).value_mut()
            }
        }
    }

    fn and_modify_with_timeout<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        self.and_modify(|v| f(v.value_mut()))
    }
}

impl<K: Ord, V> CacheMapExt<K, V> for BTreeMap<K, TimedValue<V>> {
    fn get_with_timeout<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.get(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value())
            }
        })
    }

    fn get_with_timeout_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.get_mut(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value_mut())
            }
        })
    }

    fn insert_with_timeout(&mut self, k: K, v: V, timeout: Option<Duration>) -> Option<V> {
        self.insert(k, TimedValue::new(v, timeout))
            .map(|tv| tv.into_value())
    }

    fn remove_expired_values(&mut self) {
        self.retain(|_, tv| !tv.is_expired());
    }
}

impl<'a, K, V> EntryExt<'a, K, V> for btree_map::Entry<'a, K, TimedValue<V>>
where
    K: Ord,
{
    fn or_insert_with_timeout(self, default: V, timeout: Option<Duration>) -> &'a mut V {
        match self {
            btree_map::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default, timeout);
                }
                v.value_mut()
            }
            btree_map::Entry::Vacant(entry) => {
                entry.insert(TimedValue::new(default, timeout)).value_mut()
            }
        }
    }

    fn or_insert_with_timeout_f<F: FnOnce() -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            btree_map::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default(), timeout);
                }
                v.value_mut()
            }
            btree_map::Entry::Vacant(entry) => entry
                .insert(TimedValue::new(default(), timeout))
                .value_mut(),
        }
    }

    fn or_insert_with_timeout_key_f<F: FnOnce(&K) -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            btree_map::Entry::Occupied(entry) => {
                let value = if entry.get().is_expired() {
                    Some(default(entry.key()))
                } else {
                    None
                };
                let v = entry.into_mut();
                if let Some(value) = value {
                    *v = TimedValue::new(value, timeout);
                }
                v.value_mut()
            }
            btree_map::Entry::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(TimedValue::new(value, timeout)).value_mut()
            }
        }
    }

    fn and_modify_with_timeout<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        self.and_modify(|v| f(v.value_mut()))
    }
}

impl<K: Ord + Clone, V> CacheMapExt<K, V> for dequemap::DequeBTreeMap<K, TimedValue<V>> {
    fn get_with_timeout<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.get(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value())
            }
        })
    }

    fn get_with_timeout_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.get_mut(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value_mut())
            }
        })
    }

    fn insert_with_timeout(&mut self, k: K, v: V, timeout: Option<Duration>) -> Option<V> {
        self.insert(k, TimedValue::new(v, timeout))
            .map(|tv| tv.into_value())
    }

    fn remove_expired_values(&mut self) {
        self.retain(|_, tv| !tv.is_expired());
    }
}

impl<'a, K, V> EntryExt<'a, K, V> for dequemap::btreemap::Entry<'a, K, TimedValue<V>>
where
    K: Ord + Clone,
{
    fn or_insert_with_timeout(self, default: V, timeout: Option<Duration>) -> &'a mut V {
        match self {
            dequemap::btreemap::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default, timeout);
                }
                v.value_mut()
            }
            dequemap::btreemap::Entry::Vacant(entry) => {
                entry.insert(TimedValue::new(default, timeout)).value_mut()
            }
        }
    }

    fn or_insert_with_timeout_f<F: FnOnce() -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            dequemap::btreemap::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default(), timeout);
                }
                v.value_mut()
            }
            dequemap::btreemap::Entry::Vacant(entry) => entry
                .insert(TimedValue::new(default(), timeout))
                .value_mut(),
        }
    }

    fn or_insert_with_timeout_key_f<F: FnOnce(&K) -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            dequemap::btreemap::Entry::Occupied(entry) => {
                let value = if entry.get().is_expired() {
                    Some(default(entry.key()))
                } else {
                    None
                };
                let v = entry.into_mut();
                if let Some(value) = value {
                    *v = TimedValue::new(value, timeout);
                }
                v.value_mut()
            }
            dequemap::btreemap::Entry::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(TimedValue::new(value, timeout)).value_mut()
            }
        }
    }

    fn and_modify_with_timeout<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        self.and_modify(|v| f(v.value_mut()))
    }
}

//------------------
impl<K, V, S> CacheMapExt<K, V> for dequemap::DequeHashMap<K, TimedValue<V>, S>
where
    K: Hash + Eq + Ord + Clone,
    S: BuildHasher,
{
    fn get_with_timeout<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value())
            }
        })
    }

    fn get_with_timeout_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get_mut(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value_mut())
            }
        })
    }

    fn insert_with_timeout(&mut self, k: K, v: V, timeout: Option<Duration>) -> Option<V> {
        self.insert(k, TimedValue::new(v, timeout))
            .map(|tv| tv.into_value())
    }

    fn remove_expired_values(&mut self) {
        self.retain(|_, tv| !tv.is_expired());
    }
}

impl<'a, K, V, S> EntryExt<'a, K, V> for dequemap::hashmap::Entry<'a, K, TimedValue<V>, S>
where
    K: Eq + Hash + Clone,
    S: BuildHasher,
{
    fn or_insert_with_timeout(self, default: V, timeout: Option<Duration>) -> &'a mut V {
        match self {
            dequemap::hashmap::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default, timeout);
                }
                v.value_mut()
            }
            dequemap::hashmap::Entry::Vacant(entry) => {
                entry.insert(TimedValue::new(default, timeout)).value_mut()
            }
        }
    }

    fn or_insert_with_timeout_f<F: FnOnce() -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            dequemap::hashmap::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default(), timeout);
                }
                v.value_mut()
            }
            dequemap::hashmap::Entry::Vacant(entry) => entry
                .insert(TimedValue::new(default(), timeout))
                .value_mut(),
        }
    }

    fn or_insert_with_timeout_key_f<F: FnOnce(&K) -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            dequemap::hashmap::Entry::Occupied(entry) => {
                let value = if entry.get().is_expired() {
                    Some(default(entry.key()))
                } else {
                    None
                };
                let v = entry.into_mut();
                if let Some(value) = value {
                    *v = TimedValue::new(value, timeout);
                }
                v.value_mut()
            }
            dequemap::hashmap::Entry::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(TimedValue::new(value, timeout)).value_mut()
            }
        }
    }

    fn and_modify_with_timeout<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        self.and_modify(|v| f(v.value_mut()))
    }
}

#[derive(Clone, Debug)]
pub struct TimedValue<V>(V, Option<Instant>);

impl<V> TimedValue<V> {
    pub fn new(value: V, timeout_duration: Option<Duration>) -> Self {
        TimedValue(value, timeout_duration.map(|t| Instant::now() + t))
    }

    pub fn value(&self) -> &V {
        &self.0
    }

    pub fn value_mut(&mut self) -> &mut V {
        &mut self.0
    }

    pub fn into_value(self) -> V {
        self.0
    }

    pub fn is_expired(&self) -> bool {
        self.1.map(|e| Instant::now() >= e).unwrap_or(false)
    }
}

impl<V> PartialEq for TimedValue<V>
where
    V: PartialEq,
{
    fn eq(&self, other: &TimedValue<V>) -> bool {
        self.value() == other.value()
    }
}

//impl<V> Eq for TimedValue<V> where V: Eq {}

#[test]
fn test_cache_map_ext() {
    use std::collections::hash_map::RandomState;

    let mut m: HashMap<_, _, RandomState> = HashMap::default();
    let old1 = m.insert("k1", TimedValue::new(1, None));
    let old2 = m.insert("k2", TimedValue::new(2, Some(Duration::from_millis(50))));
    let old3 = m.insert("k3", TimedValue::new(3, Some(Duration::from_millis(80))));
    let old4 = m.insert("k4", TimedValue::new(4, Some(Duration::from_millis(130))));
    let old44 = m.insert("k4", TimedValue::new(44, None));

    let old6 = m.insert_with_timeout("k6", 6, Some(Duration::from_secs(150)));
    let old7 = m.insert_with_timeout("k7", 7, None);

    let old66 = m.insert_with_timeout("k6", 66, Some(Duration::from_secs(60)));

    assert_eq!(old1, None);
    assert_eq!(old2, None);
    assert_eq!(old3, None);
    assert_eq!(old4, None);
    assert_eq!(old6, None);
    assert_eq!(old7, None);

    println!("old44: {:?}", old44);
    assert_eq!(old44, Some(TimedValue::new(4, None)));

    assert_eq!(old66, Some(6));
    println!("old66: {:?}", old66);

    let v6 = m.get("k6");
    println!("v6: {:?}", v6);
    assert_eq!(v6, Some(&TimedValue::new(66, None)));

    m.get_with_timeout_mut("k6").map(|v| *v = 666);
    let v6 = m.get_with_timeout("k6");
    println!("v6: {:?}", v6);
    assert_eq!(v6, Some(&666));

    for i in 0..20 {
        m.remove_expired_values();
        println!(
            "{} map len: {},  map k1: {:?}, k2: {:?}",
            i,
            m.len(),
            m.get("k1"),
            m.get_with_timeout("k2")
        );
        //        println!("{} map len: {},  map: {:?}", i, m.len(), m);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}

#[test]
fn test_btree_map_ext() {
    let mut m: BTreeMap<_, _> = BTreeMap::default();

    let old1 = m.insert_with_timeout("k1", 1, None);
    let old2 = m.insert_with_timeout("k2", 2, Some(Duration::from_millis(50)));
    let old3 = m.insert_with_timeout("k3", 3, Some(Duration::from_millis(80)));
    let old4 = m.insert_with_timeout("k4", 4, Some(Duration::from_millis(130)));
    let old44 = m.insert_with_timeout("k4", 44, None);

    let old6 = m.insert_with_timeout("k6", 6, Some(Duration::from_secs(150)));
    let old7 = m.insert_with_timeout("k7", 7, None);
    let old66 = m.insert_with_timeout("k6", 66, Some(Duration::from_secs(60)));

    assert_eq!(old1, None);
    assert_eq!(old2, None);
    assert_eq!(old3, None);
    assert_eq!(old4, None);
    assert_eq!(old6, None);
    assert_eq!(old7, None);

    println!("old44: {:?}", old44);
    assert_eq!(old44, Some(4));
    assert_eq!(m.get_with_timeout("k4"), Some(&44));

    assert_eq!(old66, Some(6));
    println!("old66: {:?}", old66);

    let v6 = m.get("k6");
    println!("v6: {:?}", v6);
    assert_eq!(v6, Some(&TimedValue::new(66, None)));

    m.get_with_timeout_mut("k6").map(|v| *v = 666);
    let v6 = m.get_with_timeout("k6");
    println!("v6: {:?}", v6);
    assert_eq!(v6, Some(&666));

    m.entry("kk1").or_insert_with_timeout_f(|| 10, None);
    assert_eq!(m.get_with_timeout("kk1"), Some(&10));
    m.entry("kk1").and_modify_with_timeout(|v| {
        *v = 100;
    });
    assert_eq!(m.get_with_timeout("kk1"), Some(&100));
    println!("kk1: {:?}", m.get_with_timeout("kk1"));
}

#[test]
fn test_btree_map_ext_removes() {
    let mut m: dequemap::DequeBTreeMap<_, _> = dequemap::DequeBTreeMap::default();
    m.push_back(3, TimedValue::new((), Some(Duration::from_millis(800))));
    std::thread::sleep(Duration::from_millis(100));
    m.push_back(1, TimedValue::new((), Some(Duration::from_millis(800))));
    std::thread::sleep(Duration::from_millis(100));
    m.push_back(6, TimedValue::new((), Some(Duration::from_millis(800))));
    std::thread::sleep(Duration::from_millis(100));
    m.push_back(8, TimedValue::new((), Some(Duration::from_millis(800))));
    std::thread::sleep(Duration::from_millis(100));
    m.push_back(3, TimedValue::new((), Some(Duration::from_millis(800))));

    assert_eq!(m.len(), 4);
    for (key, item) in m.iter() {
        println!("key: {:?}, is_expired: {}", key, item.is_expired());
    }
    println!("--------------------------------------------------------------");
    std::thread::sleep(Duration::from_millis(600));

    while let Some((key, item)) = m.front() {
        println!("clean expired, key: {}", key);
        if item.is_expired() {
            m.pop_front();
        } else {
            break;
        }
    }
    println!("m.len(): {}", m.len());
    for (key, item) in m.iter() {
        println!("key: {:?}, is_expired: {}", key, item.is_expired());
    }
    assert_eq!(m.len(), 2);
}

// ============== Additional tests ==============

#[test]
fn test_timed_value_new_no_expiry() {
    let mut tv = TimedValue::new(42, None);
    assert_eq!(*tv.value(), 42);
    assert_eq!(*tv.value_mut(), 42);
    assert_eq!(tv.into_value(), 42);
}

#[test]
fn test_timed_value_with_duration() {
    let tv = TimedValue::new(42, Some(std::time::Duration::from_secs(60)));
    assert_eq!(*tv.value(), 42);
    // Should not be expired immediately
    assert!(!tv.is_expired());
}

#[test]
fn test_timed_value_is_expired() {
    // Use a very short duration and immediately check — unlikely to expire
    let tv = TimedValue::new(42, Some(std::time::Duration::from_millis(1)));
    // Sleep past the timeout
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(tv.is_expired());
}

#[test]
fn test_timed_value_value_mut_modify() {
    let mut tv = TimedValue::new(1, None);
    *tv.value_mut() = 100;
    assert_eq!(*tv.value(), 100);
}

#[test]
fn test_cache_map_ext_hashmap_basic() {
    let mut m: std::collections::HashMap<&str, TimedValue<i32>> = std::collections::HashMap::new();

    m.insert_with_timeout("a", 1, None);
    m.insert_with_timeout("b", 2, Some(std::time::Duration::from_secs(60)));

    assert_eq!(m.get_with_timeout("a"), Some(&1));
    assert_eq!(m.get_with_timeout("b"), Some(&2));
    assert_eq!(m.get_with_timeout("c"), None);
}

#[test]
fn test_cache_map_ext_hashmap_overwrite() {
    let mut m: std::collections::HashMap<&str, TimedValue<i32>> = std::collections::HashMap::new();

    let old = m.insert_with_timeout("k", 1, None);
    assert_eq!(old, None);

    let old = m.insert_with_timeout("k", 2, None);
    assert_eq!(old, Some(1));
    assert_eq!(m.get_with_timeout("k"), Some(&2));
}

#[test]
fn test_cache_map_ext_hashmap_expired_returns_none() {
    let mut m: std::collections::HashMap<&str, TimedValue<i32>> = std::collections::HashMap::new();

    m.insert(
        "k",
        TimedValue::new(1, Some(std::time::Duration::from_millis(1))),
    );
    std::thread::sleep(std::time::Duration::from_millis(5));
    // Expired items are not returned
    assert_eq!(m.get_with_timeout("k"), None);
}

#[test]
fn test_cache_map_ext_hashmap_get_mut_and_set_ttl() {
    let mut m: std::collections::HashMap<&str, TimedValue<i32>> = std::collections::HashMap::new();

    m.insert_with_timeout("k", 1, None);
    m.get_with_timeout_mut("k").map(|v| *v = 42);
    assert_eq!(m.get_with_timeout("k"), Some(&42));
}

#[test]
fn test_cache_map_ext_hashmap_remove_expired() {
    let mut m: std::collections::HashMap<&str, TimedValue<i32>> = std::collections::HashMap::new();

    m.insert(
        "expired",
        TimedValue::new(1, Some(std::time::Duration::from_millis(1))),
    );
    m.insert("keep", TimedValue::new(2, None));

    std::thread::sleep(std::time::Duration::from_millis(5));
    m.remove_expired_values();
    assert_eq!(m.len(), 1);
    assert!(m.contains_key("keep"));
    assert!(!m.contains_key("expired"));
}

#[test]
fn test_entry_ext_hashmap_or_insert_with_timeout() {
    let mut m: std::collections::HashMap<&str, TimedValue<i32>> = std::collections::HashMap::new();

    // Vacant entry
    let val = m.entry("a").or_insert_with_timeout(10, None);
    assert_eq!(*val, 10);

    // Occupied entry (not expired)
    let val = m.entry("a").or_insert_with_timeout(99, None);
    assert_eq!(*val, 10);
}

#[test]
fn test_entry_ext_hashmap_or_insert_with_timeout_expired() {
    let mut m: std::collections::HashMap<&str, TimedValue<i32>> = std::collections::HashMap::new();

    m.insert(
        "a",
        TimedValue::new(1, Some(std::time::Duration::from_millis(1))),
    );
    std::thread::sleep(std::time::Duration::from_millis(5));

    // Expired entry — should be replaced
    let val = m.entry("a").or_insert_with_timeout(200, None);
    assert_eq!(*val, 200);
}

#[test]
fn test_entry_ext_hashmap_or_insert_with_timeout_f() {
    let mut m: std::collections::HashMap<&str, TimedValue<i32>> = std::collections::HashMap::new();

    let val = m.entry("a").or_insert_with_timeout_f(|| 42, None);
    assert_eq!(*val, 42);
}

#[test]
fn test_entry_ext_hashmap_or_insert_with_timeout_key_f() {
    let mut m: std::collections::HashMap<&str, TimedValue<String>> =
        std::collections::HashMap::new();

    let val = m
        .entry("a")
        .or_insert_with_timeout_key_f(|k| format!("val-{}", k), None);
    assert_eq!(**val, "val-a".to_string());
}

#[test]
fn test_entry_ext_hashmap_and_modify_with_timeout() {
    let mut m: std::collections::HashMap<&str, TimedValue<i32>> = std::collections::HashMap::new();
    m.insert("a", TimedValue::new(1, None));

    m.entry("a").and_modify_with_timeout(|v| *v = 99);
    assert_eq!(m.get_with_timeout("a"), Some(&99));
}

// ---------- BTreeMap ----------

#[test]
fn test_cache_map_ext_btreemap() {
    let mut m: std::collections::BTreeMap<&str, TimedValue<i32>> =
        std::collections::BTreeMap::new();

    m.insert_with_timeout("a", 1, None);
    m.insert_with_timeout("b", 2, Some(std::time::Duration::from_secs(60)));

    assert_eq!(m.get_with_timeout("a"), Some(&1));
    assert_eq!(m.get_with_timeout("b"), Some(&2));
    assert_eq!(m.get_with_timeout("c"), None);

    m.get_with_timeout_mut("a").map(|v| *v = 10);
    assert_eq!(m.get_with_timeout("a"), Some(&10));
}

#[test]
fn test_cache_map_ext_btreemap_remove_expired() {
    let mut m: std::collections::BTreeMap<&str, TimedValue<i32>> =
        std::collections::BTreeMap::new();

    m.insert(
        "expired",
        TimedValue::new(1, Some(std::time::Duration::from_millis(1))),
    );
    m.insert("keep", TimedValue::new(2, None));

    std::thread::sleep(std::time::Duration::from_millis(5));
    m.remove_expired_values();
    assert_eq!(m.len(), 1);
    assert!(m.contains_key("keep"));
}

#[test]
fn test_entry_ext_btreemap_or_insert_with_timeout() {
    let mut m: std::collections::BTreeMap<&str, TimedValue<i32>> =
        std::collections::BTreeMap::new();

    let val = m.entry("a").or_insert_with_timeout(10, None);
    assert_eq!(*val, 10);

    let val = m.entry("a").or_insert_with_timeout(99, None);
    assert_eq!(*val, 10);
}

#[test]
fn test_entry_ext_btreemap_or_insert_with_timeout_f() {
    let mut m: std::collections::BTreeMap<&str, TimedValue<i32>> =
        std::collections::BTreeMap::new();

    let val = m.entry("a").or_insert_with_timeout_f(|| 42, None);
    assert_eq!(*val, 42);
}

#[test]
fn test_entry_ext_btreemap_or_insert_with_timeout_key_f() {
    let mut m: std::collections::BTreeMap<String, TimedValue<String>> =
        std::collections::BTreeMap::new();

    let val = m
        .entry("key".to_string())
        .or_insert_with_timeout_key_f(|k| k.clone(), None);
    assert_eq!(**val, "key".to_string());
}

#[test]
fn test_entry_ext_btreemap_and_modify_with_timeout() {
    let mut m: std::collections::BTreeMap<&str, TimedValue<i32>> =
        std::collections::BTreeMap::new();
    m.insert("a", TimedValue::new(1, None));

    m.entry("a").and_modify_with_timeout(|v| *v = 99);
    assert_eq!(m.get_with_timeout("a"), Some(&99));
}

// ---------- DequeBTreeMap (via dequemap) ----------

#[test]
fn test_cache_map_ext_deque_btreemap() {
    let mut m: dequemap::DequeBTreeMap<&str, TimedValue<i32>> = dequemap::DequeBTreeMap::default();

    m.insert_with_timeout("a", 1, None);
    m.insert_with_timeout("b", 2, Some(std::time::Duration::from_secs(60)));

    assert_eq!(m.get_with_timeout("a"), Some(&1));
    assert_eq!(m.get_with_timeout("b"), Some(&2));
    assert_eq!(m.get_with_timeout("c"), None);

    m.get_with_timeout_mut("a").map(|v| *v = 10);
    assert_eq!(m.get_with_timeout("a"), Some(&10));
}

#[test]
fn test_entry_ext_deque_btreemap_or_insert_with_timeout() {
    let mut m: dequemap::DequeBTreeMap<&str, TimedValue<i32>> = dequemap::DequeBTreeMap::default();

    let val = m.entry("a").or_insert_with_timeout(10, None);
    assert_eq!(*val, 10);

    let val = m.entry("a").or_insert_with_timeout(99, None);
    assert_eq!(*val, 10);
}

#[test]
fn test_entry_ext_deque_btreemap_and_modify_with_timeout() {
    let mut m: dequemap::DequeBTreeMap<&str, TimedValue<i32>> = dequemap::DequeBTreeMap::default();
    m.insert("a", TimedValue::new(1, None));

    m.entry("a").and_modify_with_timeout(|v| *v = 99);
    assert_eq!(m.get_with_timeout("a"), Some(&99));
}

// ---------- DequeHashMap (via dequemap) ----------

#[test]
fn test_cache_map_ext_deque_hashmap() {
    let mut m: dequemap::DequeHashMap<&str, TimedValue<i32>> = dequemap::DequeHashMap::default();

    m.insert_with_timeout("a", 1, None);
    m.insert_with_timeout("b", 2, Some(std::time::Duration::from_secs(60)));

    assert_eq!(m.get_with_timeout("a"), Some(&1));
    assert_eq!(m.get_with_timeout("b"), Some(&2));
    assert_eq!(m.get_with_timeout("c"), None);
}

#[test]
fn test_entry_ext_deque_hashmap_or_insert_with_timeout() {
    let mut m: dequemap::DequeHashMap<&str, TimedValue<i32>> = dequemap::DequeHashMap::default();

    let val = m.entry("a").or_insert_with_timeout(10, None);
    assert_eq!(*val, 10);

    let val = m.entry("a").or_insert_with_timeout(99, None);
    assert_eq!(*val, 10);
}

#[test]
fn test_entry_ext_deque_hashmap_and_modify() {
    let mut m: dequemap::DequeHashMap<&str, TimedValue<i32>> = dequemap::DequeHashMap::default();
    m.insert("a", TimedValue::new(1, None));

    m.entry("a").and_modify_with_timeout(|v| *v = 99);
    assert_eq!(m.get_with_timeout("a"), Some(&99));
}

#[test]
fn test_timed_value_partial_eq() {
    let tv1 = TimedValue::new(42, None);
    let tv2 = TimedValue::new(42, Some(std::time::Duration::from_secs(60)));
    let tv3 = TimedValue::new(99, None);

    assert_eq!(tv1, tv2);
    assert_ne!(tv1, tv3);
}
