use std::sync::Arc;

impl<T: ?Sized> StdExt for T {}

pub trait StdExt {
    #[inline]
    fn arc(self) -> Arc<Self>
        where
            Self: Sized,
    {
        Arc::new(self)
    }
}