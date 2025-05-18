#[macro_use]
extern crate serde;

pub mod transferpb {
    tonic::include_proto!("transferpb");
}

pub type Priority = u32;
pub(crate) type Id = u64;
pub(crate) type RemovedIds = Vec<Id>;
pub mod client;
pub mod server;

pub use anyhow::{Error, Result};

#[inline]
pub(crate) fn split_into_chunks(
    id: Id,
    data: &[u8],
    p: Priority,
    chunk_size: usize,
) -> Vec<transferpb::Message> {
    let chunks: Vec<_> = data.chunks(chunk_size).collect();
    let total_chunks = chunks.len() as u32;
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| transferpb::Message {
            id,
            priority: p,
            total_chunks,
            chunk_index: i as u32,
            data: Some(chunk.into()),
            ..Default::default()
        })
        .collect()
}

use std::cmp::Reverse;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use collections::PriorityQueue;
use dequemap::DequeBTreeMap;
use tokio::sync::RwLock;

#[allow(clippy::type_complexity)]
struct ChunkedBuffer {
    data_buffses: Vec<
        RwLock<
            DequeBTreeMap<
                (Option<SocketAddr>, Id),
                (Instant, PriorityQueue<Reverse<u32>, transferpb::Message>),
            >,
        >,
    >,
    recv_chunks_timeout: Duration,
}

impl ChunkedBuffer {
    fn new(recv_chunks_timeout: Duration) -> Self {
        let data_buffses = (0..DATA_BUFFSES_MAX)
            .map(|_| RwLock::new(DequeBTreeMap::default()))
            .collect();
        ChunkedBuffer {
            data_buffses,
            recv_chunks_timeout,
        }
    }

    #[inline]
    async fn merge(
        &self,
        req: transferpb::Message,
        remote_addr: Option<SocketAddr>,
        mut removed_ids: Option<&mut RemovedIds>,
    ) -> Option<(Id, Priority, Vec<u8>)> {
        if req.total_chunks > 1 {
            let idx = req.id % DATA_BUFFSES_MAX;
            let data_buffs = if let Some(data_buffs) = self.data_buffses.get(idx as usize) {
                data_buffs
            } else {
                unreachable!();
            };

            let mut now = None;
            let mut data_buffs = data_buffs.write().await;
            while let Some((id, is_empty, is_timeout)) =
                data_buffs.front().and_then(|((_, id), (t, q))| {
                    let is_empty = q.is_empty();
                    if now.is_none() {
                        now = Some(Instant::now());
                    };
                    let is_timeout = if let Some(now) = &now {
                        now.duration_since(*t) > self.recv_chunks_timeout
                    } else {
                        false
                    };
                    if is_empty || is_timeout {
                        Some((*id, is_empty, is_timeout))
                    } else {
                        None
                    }
                })
            {
                if !is_empty && is_timeout {
                    log::warn!("Message merge timeout, message ID: {}", id)
                }
                if let Some(removed_ids) = removed_ids.as_mut() {
                    removed_ids.push(id);
                }
                data_buffs.pop_front();
            }
            let (_, data_buff) = data_buffs
                .entry((remote_addr, req.id))
                .or_insert_with(|| (Instant::now(), PriorityQueue::default()));
            let total_chunks = req.total_chunks;
            let priority = req.priority;
            let id = req.id;
            data_buff.push(Reverse(req.chunk_index), req);
            if data_buff.len() >= total_chunks as usize {
                let merged_data = data_buff
                    .drain_sorted()
                    .flat_map(|(_, msg)| msg.data.unwrap_or_default())
                    .collect::<Vec<_>>();
                Some((id, priority, merged_data))
            } else {
                None
            }
        } else {
            Some((req.id, req.priority, req.data.unwrap_or_default()))
        }
    }
}

pub(crate) const RECV_CHUNKS_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const DATA_BUFFSES_MAX: u64 = 10;

pub(crate) const CHUNK_SIZE_LIMIT: usize = 1024 * 1024;
