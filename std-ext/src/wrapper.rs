use super::{tuple_deref, tuple_deref_mut};
use std::hash::{Hash, Hasher};

impl<T: ?Sized> OrdExt for T {}

pub trait OrdExt {
    #[inline]
    fn eq_ord(self) -> EqOrdWrapper<Self>
    where
        Self: Sized,
    {
        EqOrdWrapper(self)
    }

    #[inline]
    fn neq_ord(self) -> NeqOrdWrapper<Self>
    where
        Self: Sized,
    {
        NeqOrdWrapper(self)
    }
}

pub struct NeqOrdWrapper<T>(T);

tuple_deref!(NeqOrdWrapper<T>);
tuple_deref_mut!(NeqOrdWrapper<T>);

impl<T> Eq for NeqOrdWrapper<T> {}

impl<T> PartialEq<Self> for NeqOrdWrapper<T> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl<T> PartialOrd<Self> for NeqOrdWrapper<T> {
    #[allow(clippy::non_canonical_partial_ord_impl)]
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        None
    }
}

impl<T> Ord for NeqOrdWrapper<T> {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Greater
    }
}

pub struct EqOrdWrapper<T>(T);

tuple_deref!(EqOrdWrapper<T>);
tuple_deref_mut!(EqOrdWrapper<T>);

impl<T> Eq for EqOrdWrapper<T> {}

impl<T> PartialEq<Self> for EqOrdWrapper<T> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> PartialOrd<Self> for EqOrdWrapper<T> {
    #[allow(clippy::non_canonical_partial_ord_impl)]
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        Some(std::cmp::Ordering::Equal)
    }
}

impl<T> Ord for EqOrdWrapper<T> {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
}

impl<T: ?Sized> HashExt for T {}

pub trait HashExt {
    #[inline]
    fn hash_value(self, h: u64) -> HashWrapper<Self>
    where
        Self: Sized,
    {
        HashWrapper(self, h)
    }

    #[inline]
    fn hash_empty(self) -> EmptyHashWrapper<Self>
    where
        Self: Sized,
    {
        EmptyHashWrapper(self)
    }
}

pub struct HashWrapper<T>(T, u64);

tuple_deref!(HashWrapper<T>);
tuple_deref_mut!(HashWrapper<T>);

impl<T> Hash for HashWrapper<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.1.hash(state);
    }
}

pub struct EmptyHashWrapper<T>(T);

tuple_deref!(EmptyHashWrapper<T>);
tuple_deref_mut!(EmptyHashWrapper<T>);

impl<T> Hash for EmptyHashWrapper<T> {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<T: ?Sized> OrdHashExt for T {}

pub trait OrdHashExt {
    #[inline]
    fn neq_ord_hash(self, h: u64) -> NeqOrdHashWrapper<Self>
    where
        Self: Sized,
    {
        NeqOrdHashWrapper(self, h)
    }

    #[inline]
    fn neq_ord_empty(self) -> NeqOrdEmptyHashWrapper<Self>
    where
        Self: Sized,
    {
        NeqOrdEmptyHashWrapper(self)
    }
}

pub struct NeqOrdEmptyHashWrapper<T>(T);

tuple_deref!(NeqOrdEmptyHashWrapper<T>);
tuple_deref_mut!(NeqOrdEmptyHashWrapper<T>);

impl<T> Eq for NeqOrdEmptyHashWrapper<T> {}

impl<T> PartialEq<Self> for NeqOrdEmptyHashWrapper<T> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl<T> PartialOrd<Self> for NeqOrdEmptyHashWrapper<T> {
    #[allow(clippy::non_canonical_partial_ord_impl)]
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        None
    }
}

impl<T> Ord for NeqOrdEmptyHashWrapper<T> {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Greater
    }
}

impl<T> Hash for NeqOrdEmptyHashWrapper<T> {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

pub struct NeqOrdHashWrapper<T>(T, u64);

tuple_deref!(NeqOrdHashWrapper<T>);
tuple_deref_mut!(NeqOrdHashWrapper<T>);

impl<T> Eq for NeqOrdHashWrapper<T> {}

impl<T> PartialEq<Self> for NeqOrdHashWrapper<T> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl<T> PartialOrd<Self> for NeqOrdHashWrapper<T> {
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        None
    }
}

impl<T> Hash for NeqOrdHashWrapper<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.1.hash(state)
    }
}

#[test]
fn test_neq() {
    let a1 = NeqOrdWrapper(1);
    let a2 = NeqOrdWrapper(1);
    assert!(a1.ne(&a2));
}

#[test]
fn test_eq() {
    let a1 = EqOrdWrapper(1);
    let a2 = EqOrdWrapper(1);
    assert!(a1.eq(&a2));
}
