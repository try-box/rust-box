use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{Future, Sink, SinkExt};
use futures::channel::oneshot;

use super::{Error, ErrorType, Executor};

pub struct Spawner<'a, Item> {
    sink: &'a Executor,
    item: Option<Item>,
}

impl<Item> Unpin for Spawner<'_, Item> {}

impl<'a, Item> Spawner<'a, Item> {
    #[inline]
    pub(crate) fn new(sink: &'a Executor, item: Item) -> Self {
        Self {
            sink,
            item: Some(item),
        }
    }

    // #[inline]
    // pub fn exec(mut self, exec: &'a Executor) -> Self {
    //     self.sink = exec;
    //     self
    // }

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

impl<Item> Future for Spawner<'_, Item>
    where
        Item: Future + Send + 'static,
        Item::Output: Send + 'static,
{
    type Output = Result<(), Error<Item>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let task = this.item.take().expect("polled Feed after completion");
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
