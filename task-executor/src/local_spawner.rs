use std::fmt::Debug;
use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{Future, Sink, SinkExt};
use futures::channel::oneshot;

use crate::LocalTaskType;

use super::{assert_future, Error, ErrorType, LocalExecutor};

pub struct LocalGroupSpawner<'a, Item, Tx, G> {
    inner: LocalSpawner<'a, Item, Tx, G, ()>,
    name: Option<G>,
}

impl<Item, Tx, G> Unpin for LocalGroupSpawner<'_, Item, Tx, G> {}

impl<'a, Item, Tx, G> LocalGroupSpawner<'a, Item, Tx, G>
    where
        Tx: Clone + Unpin + Sink<((), LocalTaskType)> + Sync + 'static,
        G: Hash + Eq + Clone + Debug + Sync + 'static,
{
    #[inline]
    pub(crate) fn new(inner: LocalSpawner<'a, Item, Tx, G, ()>, name: G) -> Self {
        Self {
            inner,
            name: Some(name),
        }
    }

    #[inline]
    pub async fn result(mut self) -> Result<Item::Output, Error<Item>>
        where
            Item: Future + 'static,
            Item::Output: 'static,
    {
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

        if self.inner.sink.is_closed() {
            return Err(Error::SendError(ErrorType::Closed(Some(task))));
        }

        let (res_tx, res_rx) = oneshot::channel();
        let waiting_count = self.inner.sink.waiting_count.clone();
        let task = async move {
            waiting_count.dec();
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

impl<Item, Tx, G> Future for LocalGroupSpawner<'_, Item, Tx, G>
    where
        Item: Future + 'static,
        Item::Output: 'static,
        Tx: Clone + Unpin + Sink<((), LocalTaskType)> + Sync + 'static,
        G: Hash + Eq + Clone + Debug + Sync + 'static,
{
    type Output = Result<(), Error<Item>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
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

        if this.inner.sink.is_closed() {
            return Poll::Ready(Err(Error::SendError(ErrorType::Closed(Some(task)))));
        }
        let waiting_count = this.inner.sink.waiting_count.clone();
        let task = async move {
            waiting_count.dec();
            let _ = task.await;
        };
        this.inner.sink.waiting_count.inc();

        let mut group_send = this
            .inner
            .sink
            .group_send(name, Box::new(Box::pin(task)))
            .boxed_local();
        use futures_lite::FutureExt;
        if (futures::ready!(group_send.poll(cx))).is_err() {
            this.inner.sink.waiting_count.dec();
            Poll::Ready(Err(Error::SendError(ErrorType::Closed(None))))
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

pub struct LocalSpawner<'a, Item, Tx, G, D> {
    sink: &'a LocalExecutor<Tx, G, D>,
    item: Option<Item>,
    d: Option<D>,
}

impl<'a, Item, Tx, G, D> Unpin for LocalSpawner<'a, Item, Tx, G, D> {}

impl<'a, Item, Tx, G> LocalSpawner<'a, Item, Tx, G, ()>
    where
        Tx: Clone + Unpin + Sink<((), LocalTaskType)> + Sync + 'static,
        G: Hash + Eq + Clone + Debug + Sync + 'static,
{
    #[inline]
    pub fn group(self, name: G) -> LocalGroupSpawner<'a, Item, Tx, G>
        where
            Item: Future + 'static,
            Item::Output: 'static,
    {
        let fut = LocalGroupSpawner::new(self, name);
        assert_future::<Result<(), _>, _>(fut)
    }
}

impl<'a, Item, Tx, G, D> LocalSpawner<'a, Item, Tx, G, D>
    where
        Tx: Clone + Unpin + Sink<(D, LocalTaskType)> + Sync + 'static,
        G: Hash + Eq + Clone + Debug + Sync + 'static,
{
    #[inline]
    pub(crate) fn new(sink: &'a LocalExecutor<Tx, G, D>, item: Item, d: D) -> Self {
        Self {
            sink,
            item: Some(item),
            d: Some(d),
        }
    }

    #[inline]
    pub async fn result(mut self) -> Result<Item::Output, Error<Item>>
        where
            Item: Future + 'static,
            Item::Output: 'static,
    {
        let task = self
            .item
            .take()
            .expect("polled Feed after completion, task is None!");
        let d = self
            .d
            .take()
            .expect("polled Feed after completion, d is None!");

        if self.sink.is_closed() {
            return Err(Error::SendError(ErrorType::Closed(Some(task))));
        }

        let (res_tx, res_rx) = oneshot::channel();
        let waiting_count = self.sink.waiting_count.clone();
        let task = async move {
            waiting_count.dec();
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

impl<Item, Tx, G, D> Future for LocalSpawner<'_, Item, Tx, G, D>
    where
        Item: Future + 'static,
        Item::Output: 'static,
        Tx: Clone + Unpin + Sink<(D, LocalTaskType)> + Sync + 'static,
        G: Hash + Eq + Clone + Debug + Sync + 'static,
{
    type Output = Result<(), Error<Item>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
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

        if this.sink.is_closed() {
            return Poll::Ready(Err(Error::SendError(ErrorType::Closed(Some(task)))));
        }

        let mut tx = this.sink.tx.clone();
        let mut sink = Pin::new(&mut tx);
        futures::ready!(sink.as_mut().poll_ready(cx))
            .map_err(|_| Error::SendError(ErrorType::Closed(None)))?;
        let waiting_count = this.sink.waiting_count.clone();
        let task = async move {
            waiting_count.dec();
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
