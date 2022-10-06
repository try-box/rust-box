use std::fmt::Debug;
use std::hash::Hash;
use std::marker::Unpin;

use futures::channel::mpsc;

use super::{assert_future, default, Spawner, TaskExecQueue, TaskType};

impl<T: ?Sized> SpawnExt for T where T: futures::Future {}

pub trait SpawnExt: futures::Future {
    #[inline]
    fn spawn<Tx, G>(self, queue: &TaskExecQueue<Tx, G>) -> Spawner<Self, Tx, G, ()>
        where
            Self: Sized + Send + 'static,
            Self::Output: Send + 'static,
            Tx: Clone + Unpin + futures::Sink<((), TaskType)> + Send + Sync + 'static,
            G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
    {
        let f = Spawner::new(queue, self, ());
        assert_future::<_, _>(f)
    }

    #[inline]
    fn spawn_with<Tx, G, D>(
        self,
        queue: &TaskExecQueue<Tx, G, D>,
        name: D,
    ) -> Spawner<Self, Tx, G, D>
        where
            Self: Sized + Send + 'static,
            Self::Output: Send + 'static,
            Tx: Clone + Unpin + futures::Sink<(D, TaskType)> + Send + Sync + 'static,
            G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
    {
        let f = Spawner::new(queue, self, name);
        assert_future::<_, _>(f)
    }
}

impl<T: ?Sized> SpawnDefaultExt for T where T: futures::Future {}

pub trait SpawnDefaultExt: futures::Future {
    #[inline]
    fn spawn(self) -> Spawner<'static, Self, mpsc::Sender<((), TaskType)>, (), ()>
        where
            Self: Sized + Send + 'static,
            Self::Output: Send + 'static,
    {
        let f = Spawner::new(default(), self, ());
        assert_future::<_, _>(f)
    }
}

pub struct Builder {
    workers: usize,
    queue_max: usize,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            workers: 100,
            queue_max: 100_000,
        }
    }
}

impl Builder {
    #[inline]
    pub fn workers(mut self, workers: usize) -> Self {
        self.workers = workers;
        self
    }

    #[inline]
    pub fn queue_max(mut self, max: usize) -> Self {
        self.queue_max = max;
        self
    }

    #[inline]
    pub fn group(self) -> GroupBuilder {
        GroupBuilder { builder: self }
    }

    #[inline]
    pub fn with_channel<Tx, Rx, D>(self, tx: Tx, rx: Rx) -> ChannelBuilder<Tx, Rx, D>
        where
            Tx: Clone + futures::Sink<(D, TaskType)> + Unpin + Send + Sync + 'static,
            Rx: futures::Stream<Item=(D, TaskType)> + Unpin,
    {
        ChannelBuilder {
            builder: self,
            tx,
            rx,
            _d: std::marker::PhantomData,
        }
    }

    #[inline]
    pub fn build(self) -> (TaskExecQueue, impl futures::Future<Output=()>) {
        let (tx, rx) = mpsc::channel(self.queue_max);
        TaskExecQueue::with_channel(self.workers, self.queue_max, tx, rx)
    }
}

pub struct GroupBuilder {
    builder: Builder,
}

impl GroupBuilder {
    #[inline]
    pub fn build<G>(
        self,
    ) -> (
        TaskExecQueue<mpsc::Sender<((), TaskType)>, G>,
        impl futures::Future<Output=()>,
    )
        where
            G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
    {
        let (tx, rx) = mpsc::channel(self.builder.queue_max);
        TaskExecQueue::with_channel(self.builder.workers, self.builder.queue_max, tx, rx)
    }
}

pub struct ChannelBuilder<Tx, Rx, D> {
    builder: Builder,
    tx: Tx,
    rx: Rx,
    _d: std::marker::PhantomData<D>,
}

impl<Tx, Rx, D> ChannelBuilder<Tx, Rx, D>
    where
        Tx: Clone + futures::Sink<(D, TaskType)> + Unpin + Send + Sync + 'static,
        Rx: futures::Stream<Item=(D, TaskType)> + Unpin,
{
    #[inline]
    pub fn build(self) -> (TaskExecQueue<Tx, (), D>, impl futures::Future<Output=()>) {
        TaskExecQueue::with_channel(
            self.builder.workers,
            self.builder.queue_max,
            self.tx,
            self.rx,
        )
    }

    #[inline]
    pub fn group(self) -> GroupChannelBuilder<Tx, Rx, D> {
        GroupChannelBuilder { builder: self }
    }
}

pub struct GroupChannelBuilder<Tx, Rx, D> {
    builder: ChannelBuilder<Tx, Rx, D>,
}

impl<Tx, Rx, D> GroupChannelBuilder<Tx, Rx, D>
    where
        Tx: Clone + futures::Sink<((), TaskType)> + Unpin + Send + Sync + 'static,
        Rx: futures::Stream<Item=((), TaskType)> + Unpin,
{
    #[inline]
    pub fn build<G>(self) -> (TaskExecQueue<Tx, G>, impl futures::Future<Output=()>)
        where
            G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
    {
        TaskExecQueue::with_channel(
            self.builder.builder.workers,
            self.builder.builder.queue_max,
            self.builder.tx,
            self.builder.rx,
        )
    }
}
