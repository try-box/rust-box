use std::fmt::Debug;
use std::hash::Hash;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::exec::PendingOnce;
use futures::channel::oneshot;
use futures::task::AtomicWaker;
use futures::{Future, Sink, SinkExt};
use futures_lite::FutureExt;

use crate::TaskType;

use super::{assert_future, Error, ErrorType, TaskExecQueue};

pub struct GroupSpawner<'a, Item, Tx, G> {
    inner: Spawner<'a, Item, Tx, G, ()>,
    name: Option<G>,
}

impl<Item, Tx, G> Unpin for GroupSpawner<'_, Item, Tx, G> {}

impl<'a, Item, Tx, G> GroupSpawner<'a, Item, Tx, G>
where
    Tx: Clone + Unpin + Sink<((), TaskType)> + Send + Sync + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    #[inline]
    pub(crate) fn new(inner: Spawner<'a, Item, Tx, G, ()>, name: G) -> Self {
        Self {
            inner,
            name: Some(name),
        }
    }

    #[inline]
    pub fn quickly(mut self) -> Self {
        self.inner.quickly = true;
        self
    }

    #[inline]
    pub async fn result(mut self) -> Result<Item::Output, Error<Item>>
    where
        Item: Future + Send + 'static,
        Item::Output: Send + 'static,
    {
        if self.inner.sink.is_closed() {
            return Err(Error::SendError(ErrorType::Closed(self.inner.item.take())));
        }

        if !self.inner.quickly && self.inner.sink.is_full() {
            let w = Arc::new(AtomicWaker::new());
            self.inner.sink.waiting_wakers.push(w.clone());
            PendingOnce::new(w).await;
        }

        let task = match self.inner.item.take() {
            Some(task) => task,
            None => {
                log::error!("polled Feed after completion, task is None!");
                return Err(Error::SendError(ErrorType::Closed(None)));
            }
        };

        let name = match self.name.take() {
            Some(name) => name,
            None => {
                log::error!("polled Feed after completion, name is None!");
                return Err(Error::SendError(ErrorType::Closed(None)));
            }
        };

        let (res_tx, res_rx) = oneshot::channel();
        let waiting_count = self.inner.sink.waiting_count.clone();
        let waiting_wakers = self.inner.sink.waiting_wakers.clone();
        let task = async move {
            waiting_count.dec();
            if let Some(w) = waiting_wakers.pop() {
                w.wake();
            }
            let output = task.await;
            if let Err(_e) = res_tx.send(output) {
                log::warn!("send result failed");
            }
        };
        self.inner.sink.waiting_count.inc();

        if let Err(_e) = self
            .inner
            .sink
            .group_send(name, Box::new(Box::pin(task)))
            .await
        {
            self.inner.sink.waiting_count.dec();
            Err(Error::SendError(ErrorType::Closed(None)))
        } else {
            res_rx.await.map_err(|_| {
                self.inner.sink.waiting_count.dec();
                Error::RecvResultError
            })
        }
    }
}

impl<Item, Tx, G> Future for GroupSpawner<'_, Item, Tx, G>
where
    Item: Future + Send + 'static,
    Item::Output: Send + 'static,
    Tx: Clone + Unpin + Sink<((), TaskType)> + Send + Sync + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    type Output = Result<(), Error<Item>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.inner.sink.is_closed() && !this.inner.is_pending {
            return Poll::Ready(Err(Error::SendError(ErrorType::Closed(
                this.inner.item.take(),
            ))));
        }

        if !this.inner.quickly && this.inner.sink.is_full() {
            let w = Arc::new(AtomicWaker::new());
            w.register(cx.waker());
            this.inner.sink.waiting_wakers.push(w);
            this.inner.is_pending = true;
            return Poll::Pending;
        }

        let task = match this.inner.item.take() {
            Some(task) => task,
            None => {
                log::error!("polled Feed after completion, task is None!");
                return Poll::Ready(Ok(()));
            }
        };

        let name = match this.name.take() {
            Some(name) => name,
            None => {
                log::error!("polled Feed after completion, name is None!");
                return Poll::Ready(Ok(()));
            }
        };

        let waiting_count = this.inner.sink.waiting_count.clone();
        let waiting_wakers = this.inner.sink.waiting_wakers.clone();
        let task = async move {
            waiting_count.dec();
            if let Some(w) = waiting_wakers.pop() {
                w.wake();
            }
            let _ = task.await;
        };
        this.inner.sink.waiting_count.inc();

        let mut group_send = this
            .inner
            .sink
            .group_send(name, Box::new(Box::pin(task)))
            .boxed();

        if (futures::ready!(group_send.poll(cx))).is_err() {
            this.inner.sink.waiting_count.dec();
            Poll::Ready(Err(Error::SendError(ErrorType::Closed(None))))
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

pub struct TryGroupSpawner<'a, Item, Tx, G> {
    inner: GroupSpawner<'a, Item, Tx, G>,
}

impl<Item, Tx, G> Unpin for TryGroupSpawner<'_, Item, Tx, G> {}

impl<'a, Item, Tx, G> TryGroupSpawner<'a, Item, Tx, G>
where
    Tx: Clone + Unpin + Sink<((), TaskType)> + Send + Sync + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    #[inline]
    pub(crate) fn new(inner: Spawner<'a, Item, Tx, G, ()>, name: G) -> Self {
        Self {
            inner: GroupSpawner {
                inner,
                name: Some(name),
            },
        }
    }

    #[inline]
    pub async fn result(mut self) -> Result<Item::Output, Error<Item>>
    where
        Item: Future + Send + 'static,
        Item::Output: Send + 'static,
    {
        if self.inner.inner.sink.is_full() {
            return Err(Error::TrySendError(ErrorType::Full(
                self.inner.inner.item.take(),
            )));
        }
        self.inner.result().await
    }
}

impl<Item, Tx, G> Future for TryGroupSpawner<'_, Item, Tx, G>
where
    Item: Future + Send + 'static,
    Item::Output: Send + 'static,
    Tx: Clone + Unpin + Sink<((), TaskType)> + Send + Sync + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    type Output = Result<(), Error<Item>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        if this.inner.inner.sink.is_full() {
            return Poll::Ready(Err(Error::TrySendError(ErrorType::Full(
                this.inner.inner.item.take(),
            ))));
        }

        this.inner.poll(cx)
    }
}

pub struct Spawner<'a, Item, Tx, G, D> {
    sink: &'a TaskExecQueue<Tx, G, D>,
    item: Option<Item>,
    d: Option<D>,
    quickly: bool,
    is_pending: bool,
}

impl<'a, Item, Tx, G, D> Unpin for Spawner<'a, Item, Tx, G, D> {}

impl<'a, Item, Tx, G> Spawner<'a, Item, Tx, G, ()>
where
    Tx: Clone + Unpin + Sink<((), TaskType)> + Send + Sync + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    #[inline]
    pub fn group(self, name: G) -> GroupSpawner<'a, Item, Tx, G>
    where
        Item: Future + Send + 'static,
        Item::Output: Send + 'static,
    {
        let fut = GroupSpawner::new(self, name);
        assert_future::<Result<(), _>, _>(fut)
    }
}

impl<'a, Item, Tx, G, D> Spawner<'a, Item, Tx, G, D>
where
    Tx: Clone + Unpin + Sink<(D, TaskType)> + Send + Sync + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    #[inline]
    pub(crate) fn new(sink: &'a TaskExecQueue<Tx, G, D>, item: Item, d: D) -> Self {
        Self {
            sink,
            item: Some(item),
            d: Some(d),
            quickly: false,
            is_pending: false,
        }
    }

    #[inline]
    pub fn quickly(mut self) -> Self {
        self.quickly = true;
        self
    }

    #[inline]
    pub async fn result(mut self) -> Result<Item::Output, Error<Item>>
    where
        Item: Future + Send + 'static,
        Item::Output: Send + 'static,
    {
        if self.sink.is_closed() {
            return Err(Error::SendError(ErrorType::Closed(self.item.take())));
        }

        if !self.quickly && self.sink.is_full() {
            let w = Arc::new(AtomicWaker::new());
            self.sink.waiting_wakers.push(w.clone());
            PendingOnce::new(w).await;
        }

        let task = self
            .item
            .take()
            .expect("polled Feed after completion, task is None!");
        let d = self
            .d
            .take()
            .expect("polled Feed after completion, d is None!");

        let (res_tx, res_rx) = oneshot::channel();
        let waiting_count = self.sink.waiting_count.clone();
        let waiting_wakers = self.sink.waiting_wakers.clone();
        let task = async move {
            waiting_count.dec();
            if let Some(w) = waiting_wakers.pop() {
                w.wake();
            }
            let output = task.await;
            if let Err(_e) = res_tx.send(output) {
                log::warn!("send result failed");
            }
        };

        self.sink.waiting_count.inc();
        if self
            .sink
            .tx
            .clone()
            .send((d, Box::new(Box::pin(task))))
            .await
            .is_err()
        {
            self.sink.waiting_count.dec();
            return Err(Error::SendError(ErrorType::Closed(None)));
        }
        res_rx.await.map_err(|_| {
            self.sink.waiting_count.dec();
            Error::RecvResultError
        })
    }
}

impl<Item, Tx, G, D> Future for Spawner<'_, Item, Tx, G, D>
where
    Item: Future + Send + 'static,
    Item::Output: Send + 'static,
    Tx: Clone + Unpin + Sink<(D, TaskType)> + Send + Sync + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    type Output = Result<(), Error<Item>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        if this.sink.is_closed() && !this.is_pending {
            return Poll::Ready(Err(Error::SendError(ErrorType::Closed(this.item.take()))));
        }

        if !this.quickly && this.sink.is_full() {
            let w = Arc::new(AtomicWaker::new());
            w.register(cx.waker());
            this.sink.waiting_wakers.push(w);
            this.is_pending = true;
            return Poll::Pending;
        }

        let task = match this.item.take() {
            Some(task) => task,
            None => {
                log::error!("polled Feed after completion, task is None!");
                return Poll::Ready(Ok(()));
            }
        };

        let d = match this.d.take() {
            Some(d) => d,
            None => {
                log::error!("polled Feed after completion, d is None!");
                return Poll::Ready(Ok(()));
            }
        };

        let mut tx = this.sink.tx.clone();
        let mut sink = Pin::new(&mut tx);

        let waiting_count = this.sink.waiting_count.clone();
        let waiting_wakers = this.sink.waiting_wakers.clone();
        let task = async move {
            waiting_count.dec();
            if let Some(w) = waiting_wakers.pop() {
                w.wake();
            }
            let _ = task.await;
        };
        this.sink.waiting_count.inc();
        sink.as_mut()
            .start_send((d, Box::new(Box::pin(task))))
            .map_err(|_e| {
                this.sink.waiting_count.dec();
                Error::SendError(ErrorType::Closed(None))
            })?;
        Poll::Ready(Ok(()))
    }
}

pub struct TrySpawner<'a, Item, Tx, G, D> {
    inner: Spawner<'a, Item, Tx, G, D>,
}

impl<'a, Item, Tx, G, D> Unpin for TrySpawner<'a, Item, Tx, G, D> {}

impl<'a, Item, Tx, G> TrySpawner<'a, Item, Tx, G, ()>
where
    Tx: Clone + Unpin + Sink<((), TaskType)> + Send + Sync + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    #[inline]
    pub fn group(self, name: G) -> TryGroupSpawner<'a, Item, Tx, G>
    where
        Item: Future + Send + 'static,
        Item::Output: Send + 'static,
    {
        let fut = TryGroupSpawner::new(self.inner, name);
        assert_future::<Result<(), _>, _>(fut)
    }
}

impl<'a, Item, Tx, G, D> TrySpawner<'a, Item, Tx, G, D>
where
    Tx: Clone + Unpin + Sink<(D, TaskType)> + Send + Sync + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    #[inline]
    pub(crate) fn new(sink: &'a TaskExecQueue<Tx, G, D>, item: Item, d: D) -> Self {
        Self {
            inner: Spawner {
                sink,
                item: Some(item),
                d: Some(d),
                quickly: false,
                is_pending: false,
            },
        }
    }

    #[inline]
    pub fn quickly(mut self) -> Self {
        self.inner.quickly = true;
        self
    }

    #[inline]
    pub async fn result(mut self) -> Result<Item::Output, Error<Item>>
    where
        Item: Future + Send + 'static,
        Item::Output: Send + 'static,
    {
        if self.inner.sink.is_full() {
            return Err(Error::TrySendError(ErrorType::Full(self.inner.item.take())));
        }
        self.inner.result().await
    }
}

impl<Item, Tx, G, D> Future for TrySpawner<'_, Item, Tx, G, D>
where
    Item: Future + Send + 'static,
    Item::Output: Send + 'static,
    Tx: Clone + Unpin + Sink<(D, TaskType)> + Send + Sync + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    type Output = Result<(), Error<Item>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.inner.sink.is_full() {
            return Poll::Ready(Err(Error::TrySendError(ErrorType::Full(
                this.inner.item.take(),
            ))));
        }
        this.inner.poll(cx)
    }
}
