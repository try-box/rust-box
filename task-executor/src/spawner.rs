use std::fmt::Debug;
use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::channel::oneshot;
use futures::{Future, Sink, SinkExt};

use super::{assert_future, Error, ErrorType, Executor};

pub struct GroupSpawner<'a, Item, G> {
    inner: Spawner<'a, Item, G>,
    name: Option<G>,
}

impl<Item, G> Unpin for GroupSpawner<'_, Item, G> {}

impl<'a, Item, G> GroupSpawner<'a, Item, G>
where
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    #[inline]
    pub(crate) fn new(inner: Spawner<'a, Item, G>, name: G) -> Self {
        Self {
            inner,
            name: Some(name),
        }
    }

    #[inline]
    pub async fn result(mut self) -> Result<Item::Output, Error<Item>>
    where
        Item: Future + Send + 'static,
        Item::Output: Send + 'static,
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
            res_rx.await.map_err(|_| Error::RecvResultError)
        }
    }
}

impl<Item, G> Future for GroupSpawner<'_, Item, G>
where
    Item: Future + Send + 'static,
    Item::Output: Send + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
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
            .boxed();
        use futures_lite::FutureExt;
        if (futures::ready!(group_send.poll(cx))).is_err() {
            this.inner.sink.waiting_count.dec();
            Poll::Ready(Err(Error::SendError(ErrorType::Closed(None))))
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

pub struct Spawner<'a, Item, G> {
    sink: &'a Executor<G>,
    item: Option<Item>,
}

impl<Item, G> Unpin for Spawner<'_, Item, G> {}

impl<'a, Item, G> Spawner<'a, Item, G>
where
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    #[inline]
    pub(crate) fn new(sink: &'a Executor<G>, item: Item) -> Self {
        Self {
            sink,
            item: Some(item),
        }
    }

    #[inline]
    pub fn group(self, name: G) -> GroupSpawner<'a, Item, G>
    where
        Item: Future + Send + 'static,
        Item::Output: Send + 'static,
    {
        let fut = GroupSpawner::new(self, name);
        assert_future::<Result<(), _>, _>(fut)
    }

    #[inline]
    pub async fn result(mut self) -> Result<Item::Output, Error<Item>>
    where
        Item: Future + Send + 'static,
        Item::Output: Send + 'static,
    {
        let task = self.item.take().expect("polled Feed after completion");

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

        if let Err(e) = self.sink.tx.clone().send(Box::new(Box::pin(task))).await {
            self.sink.waiting_count.dec();
            return Err(Error::from(e));
        }
        res_rx.await.map_err(|_| Error::RecvResultError)
    }
}

impl<Item, G> Future for Spawner<'_, Item, G>
where
    Item: Future + Send + 'static,
    Item::Output: Send + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
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
        if this.sink.is_closed() {
            return Poll::Ready(Err(Error::SendError(ErrorType::Closed(Some(task)))));
        }
        let mut tx = this.sink.tx.clone();
        let mut sink = Pin::new(&mut tx);
        futures::ready!(sink.as_mut().poll_ready(cx))?;
        let waiting_count = this.sink.waiting_count.clone();
        let task = async move {
            waiting_count.dec();
            let _ = task.await;
        };
        this.sink.waiting_count.inc();
        sink.as_mut()
            .start_send(Box::new(Box::pin(task)))
            .map_err(|e| {
                this.sink.waiting_count.dec();
                e
            })?;
        Poll::Ready(Ok(()))
    }
}
