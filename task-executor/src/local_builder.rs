use std::fmt::Debug;
use std::hash::Hash;
use std::marker::Unpin;

use super::{assert_future, LocalExecutor, LocalSpawner, LocalTaskType};

impl<T: ?Sized> LocalSpawnExt for T where T: futures::Future {}

pub trait LocalSpawnExt: futures::Future {
    #[inline]
    fn spawn<Tx, G>(self, exec: &LocalExecutor<Tx, G>) -> LocalSpawner<Self, Tx, G, ()>
        where
            Self: Sized + 'static,
            Self::Output: 'static,
            Tx: Clone + Unpin + futures::Sink<((), LocalTaskType)> + Sync + 'static,
            G: Hash + Eq + Clone + Debug + Sync + 'static,
    {
        let f = LocalSpawner::new(exec, self, ());
        assert_future::<_, _>(f)
    }

    #[inline]
    fn spawn_with<Tx, G, D>(
        self,
        exec: &LocalExecutor<Tx, G, D>,
        name: D,
    ) -> LocalSpawner<Self, Tx, G, D>
        where
            Self: Sized + 'static,
            Self::Output: 'static,
            Tx: Clone + Unpin + futures::Sink<(D, LocalTaskType)> + Sync + 'static,
            G: Hash + Eq + Clone + Debug + Sync + 'static,
    {
        let f = LocalSpawner::new(exec, self, name);
        assert_future::<_, _>(f)
    }
}

pub struct LocalBuilder {
    workers: usize,
    queue_max: usize,
}

impl Default for LocalBuilder {
    fn default() -> Self {
        Self {
            workers: 100,
            queue_max: 100_000,
        }
    }
}

impl LocalBuilder {
    #[inline]
    pub fn workers(mut self, workers: usize) -> Self {
        self.workers = workers;
        self
    }

    #[inline]
    pub fn queue_max(mut self, queue_max: usize) -> Self {
        self.queue_max = queue_max;
        self
    }

    #[inline]
    pub fn with_channel<Tx, Rx, D>(self, tx: Tx, rx: Rx) -> ChannelLocalBuilder<Tx, Rx, D>
        where
            Tx: Clone + futures::Sink<(D, LocalTaskType)> + Unpin + Sync + 'static,
            Rx: futures::Stream<Item=(D, LocalTaskType)> + Unpin,
    {
        ChannelLocalBuilder {
            builder: self,
            tx,
            rx,
            _d: std::marker::PhantomData,
        }
    }
}

pub struct ChannelLocalBuilder<Tx, Rx, D> {
    builder: LocalBuilder,
    tx: Tx,
    rx: Rx,
    _d: std::marker::PhantomData<D>,
}

impl<Tx, Rx, D> ChannelLocalBuilder<Tx, Rx, D>
    where
        Tx: Clone + futures::Sink<(D, LocalTaskType)> + Unpin + Sync + 'static,
        Rx: futures::Stream<Item=(D, LocalTaskType)> + Unpin,
{
    #[inline]
    pub fn build(self) -> (LocalExecutor<Tx, (), D>, impl futures::Future<Output=()>) {
        LocalExecutor::with_channel(
            self.builder.workers,
            self.builder.queue_max,
            self.tx,
            self.rx,
        )
    }

    #[inline]
    pub fn group(self) -> GroupChannelLocalBuilder<Tx, Rx, D> {
        GroupChannelLocalBuilder { builder: self }
    }
}

pub struct GroupChannelLocalBuilder<Tx, Rx, D> {
    builder: ChannelLocalBuilder<Tx, Rx, D>,
}

impl<Tx, Rx, D> GroupChannelLocalBuilder<Tx, Rx, D>
    where
        Tx: Clone + futures::Sink<((), LocalTaskType)> + Unpin + Sync + 'static,
        Rx: futures::Stream<Item=((), LocalTaskType)> + Unpin,
{
    #[inline]
    pub fn build<G>(self) -> (LocalExecutor<Tx, G>, impl futures::Future<Output=()>)
        where
            G: Hash + Eq + Clone + Debug + Sync + 'static,
    {
        LocalExecutor::with_channel(
            self.builder.builder.workers,
            self.builder.builder.queue_max,
            self.builder.tx,
            self.builder.rx,
        )
    }
}
