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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(
        id: Id,
        priority: Priority,
        total_chunks: u32,
        chunk_index: u32,
        data: &[u8],
    ) -> transferpb::Message {
        transferpb::Message {
            id,
            priority,
            total_chunks,
            chunk_index,
            data: Some(data.to_vec()),
            ..Default::default()
        }
    }

    // ===== split_into_chunks tests =====

    #[test]
    fn test_split_into_chunks_small_data() {
        let data = b"hello world";
        let chunks = split_into_chunks(42, data, 5, 1024);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].id, 42);
        assert_eq!(chunks[0].priority, 5);
        assert_eq!(chunks[0].total_chunks, 1);
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[0].data.as_deref(), Some(&data[..]));
    }

    #[test]
    fn test_split_into_chunks_large_data() {
        let id = 100;
        let data: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();
        let chunk_size = 1024;
        let chunks = split_into_chunks(id, &data, 0, chunk_size);
        // ceil(5000/1024) = 5
        assert_eq!(chunks.len(), 5);
        for chunk in &chunks {
            assert_eq!(chunk.id, id);
            assert_eq!(chunk.total_chunks, 5);
        }
        // Verify all data merged correctly
        let mut merged = Vec::new();
        for chunk in chunks {
            merged.extend_from_slice(chunk.data.as_deref().unwrap());
        }
        assert_eq!(merged, data);
    }

    #[test]
    fn test_split_into_chunks_empty_data() {
        let data: &[u8] = &[];
        let chunks = split_into_chunks(1, data, 0, 64);
        // data.chunks(64) on empty slice produces 0 chunks
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_split_into_chunks_exact_boundary() {
        let data = vec![42u8; 1024];
        let chunks = split_into_chunks(2, &data, 0, 1024);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].data.as_deref(), Some(&data[..]));
    }

    #[test]
    fn test_split_into_chunks_multiple_of_chunk_size() {
        let data = vec![7u8; 2048];
        let chunks = split_into_chunks(3, &data, 1, 1024);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].total_chunks, 2);
        assert_eq!(chunks[1].total_chunks, 2);
        let mut merged = Vec::new();
        for chunk in chunks {
            merged.extend_from_slice(chunk.data.as_deref().unwrap());
        }
        assert_eq!(merged, data);
    }

    #[test]
    fn test_split_into_chunks_preserves_chunk_indices() {
        let data = vec![0u8; 2500];
        let chunks = split_into_chunks(4, &data, 0, 1000);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[1].chunk_index, 1);
        assert_eq!(chunks[2].chunk_index, 2);
        // First two chunks are full 1000 bytes, last is 500
        assert_eq!(chunks[0].data.as_ref().unwrap().len(), 1000);
        assert_eq!(chunks[1].data.as_ref().unwrap().len(), 1000);
        assert_eq!(chunks[2].data.as_ref().unwrap().len(), 500);
    }

    #[test]
    fn test_split_into_chunks_different_ids() {
        let data = b"test data";
        let chunks_a = split_into_chunks(10, data, 0, 1024);
        let chunks_b = split_into_chunks(20, data, 0, 1024);
        assert_eq!(chunks_a[0].id, 10);
        assert_eq!(chunks_b[0].id, 20);
    }

    #[test]
    fn test_split_into_chunks_different_priorities() {
        let data = b"priority test";
        let chunks = split_into_chunks(5, data, 99, 1024);
        assert_eq!(chunks[0].priority, 99);
    }

    // ===== ChunkedBuffer::merge tests =====

    #[tokio::test]
    async fn test_chunked_buffer_single_chunk() {
        let buf = ChunkedBuffer::new(Duration::from_secs(30));
        let msg = make_msg(1, 0, 1, 0, b"single");
        let result = buf.merge(msg, None, None).await;
        assert!(result.is_some());
        let (id, priority, data) = result.unwrap();
        assert_eq!(id, 1);
        assert_eq!(priority, 0);
        assert_eq!(data, b"single");
    }

    #[tokio::test]
    async fn test_chunked_buffer_single_chunk_zero_total_chunks() {
        let buf = ChunkedBuffer::new(Duration::from_secs(30));
        let msg = make_msg(1, 0, 0, 0, b"no-chunks");
        let result = buf.merge(msg, None, None).await;
        assert!(result.is_some());
        let (id, _priority, data) = result.unwrap();
        assert_eq!(id, 1);
        assert_eq!(data, b"no-chunks");
    }

    #[tokio::test]
    async fn test_chunked_buffer_ordered_chunks() {
        let buf = ChunkedBuffer::new(Duration::from_secs(30));
        // Send chunk 0 first
        let r1 = buf.merge(make_msg(10, 1, 3, 0, b"aaa"), None, None).await;
        assert!(r1.is_none(), "chunk 0 should not complete yet");
        // Send chunk 1
        let r2 = buf.merge(make_msg(10, 1, 3, 1, b"bbb"), None, None).await;
        assert!(r2.is_none(), "chunk 1 should not complete yet");
        // Send chunk 2 — all 3 chunks now present
        let r3 = buf.merge(make_msg(10, 1, 3, 2, b"ccc"), None, None).await;
        assert!(r3.is_some(), "chunk 2 should complete");
        let (id, priority, data) = r3.unwrap();
        assert_eq!(id, 10);
        assert_eq!(priority, 1);
        assert_eq!(data, b"aaabbbccc");
    }

    #[tokio::test]
    async fn test_chunked_buffer_out_of_order_chunks() {
        let buf = ChunkedBuffer::new(Duration::from_secs(30));
        // Send chunk 2 first
        let r1 = buf.merge(make_msg(20, 2, 3, 2, b"ccc"), None, None).await;
        assert!(r1.is_none());
        // Send chunk 0
        let r2 = buf.merge(make_msg(20, 2, 3, 0, b"aaa"), None, None).await;
        assert!(r2.is_none());
        // Send chunk 1 — all chunks present
        let r3 = buf.merge(make_msg(20, 2, 3, 1, b"bbb"), None, None).await;
        assert!(r3.is_some());
        let (id, priority, data) = r3.unwrap();
        assert_eq!(id, 20);
        assert_eq!(priority, 2);
        // Data must be in order by chunk_index
        assert_eq!(data, b"aaabbbccc");
    }

    #[tokio::test]
    async fn test_chunked_buffer_multiple_ids_independent() {
        let buf = ChunkedBuffer::new(Duration::from_secs(30));
        // Two different messages being received concurrently
        let r1 = buf
            .merge(make_msg(30, 0, 2, 0, b"msgA-part0"), None, None)
            .await;
        assert!(r1.is_none());
        let r2 = buf
            .merge(make_msg(40, 0, 2, 0, b"msgB-part0"), None, None)
            .await;
        assert!(r2.is_none());
        // Complete msgB
        let r3 = buf
            .merge(make_msg(40, 0, 2, 1, b"msgB-part1"), None, None)
            .await;
        assert!(r3.is_some());
        let (id, _, data) = r3.unwrap();
        assert_eq!(id, 40);
        assert_eq!(data, b"msgB-part0msgB-part1");
        // Complete msgA
        let r4 = buf
            .merge(make_msg(30, 0, 2, 1, b"msgA-part1"), None, None)
            .await;
        assert!(r4.is_some());
        let (id, _, data) = r4.unwrap();
        assert_eq!(id, 30);
        assert_eq!(data, b"msgA-part0msgA-part1");
    }

    #[tokio::test]
    async fn test_chunked_buffer_timeout_clears_stale() {
        // Use a very short timeout so entries expire quickly
        let buf = ChunkedBuffer::new(Duration::from_millis(10));
        let _ = buf.merge(make_msg(50, 0, 2, 0, b"stale"), None, None).await;
        // Wait for timeout to pass
        tokio::time::sleep(Duration::from_millis(20)).await;
        // Insert a new message with a different id (higher id = back of deque)
        // The front entry (id=50) should be timed out and cleared
        let r = buf.merge(make_msg(60, 0, 1, 0, b"fresh"), None, None).await;
        assert!(r.is_some());
        let (id, _, data) = r.unwrap();
        assert_eq!(id, 60);
        assert_eq!(data, b"fresh");
    }

    #[tokio::test]
    async fn test_chunked_buffer_removed_ids_tracking() {
        let buf = ChunkedBuffer::new(Duration::from_millis(50));
        let mut removed_ids = Vec::new();
        let _ = buf
            .merge(
                make_msg(70, 0, 2, 0, b"stale"),
                None,
                Some(&mut removed_ids),
            )
            .await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Trigger timeout cleanup by inserting another message
        let _ = buf
            .merge(
                make_msg(80, 0, 2, 0, b"trigger"),
                None,
                Some(&mut removed_ids),
            )
            .await;
        assert!(
            removed_ids.contains(&70),
            "stale entry id=70 should be in removed_ids"
        );
    }

    #[tokio::test]
    async fn test_chunked_buffer_many_chunks() {
        let buf = ChunkedBuffer::new(Duration::from_secs(30));
        let total = 20;
        let chunks: Vec<_> = (0..total)
            .map(|i| make_msg(90, 0, total, i, &[i as u8]))
            .collect();
        // Send in reverse order
        for msg in chunks.into_iter().rev() {
            let r = buf.merge(msg, None, None).await;
            if r.is_some() {
                let (id, _, data) = r.unwrap();
                assert_eq!(id, 90);
                assert_eq!(data.len(), total as usize);
                for (i, &byte) in data.iter().enumerate() {
                    assert_eq!(byte, i as u8, "byte at index {} mismatch", i);
                }
                return;
            }
        }
        panic!("merge never completed");
    }

    // ===== transferpb::Message construction tests =====

    #[test]
    fn test_transferpb_message_default() {
        let msg = transferpb::Message::default();
        assert_eq!(msg.id, 0);
        assert_eq!(msg.priority, 0);
        assert_eq!(msg.total_chunks, 0);
        assert_eq!(msg.chunk_index, 0);
        assert!(msg.data.is_none());
        assert!(msg.err.is_none());
    }

    #[test]
    fn test_transferpb_message_with_data() {
        let msg = transferpb::Message {
            id: 123,
            priority: 5,
            total_chunks: 3,
            chunk_index: 1,
            data: Some(vec![1, 2, 3]),
            ..Default::default()
        };
        assert_eq!(msg.id, 123);
        assert_eq!(msg.data.unwrap(), vec![1, 2, 3]);
    }

    // ===== Re-export verification tests =====

    /// Compile-time check that client and server types are accessible
    #[test]
    fn test_client_types_accessible() {
        let _builder = client::Client::new("http://localhost:50051".to_string());
        // Verify Mailbox and SendError types also compile
        let _mailbox: std::marker::PhantomData<client::Mailbox> = std::marker::PhantomData;
        let _err: client::SendError<()> = client::SendError::disconnected(None);
    }

    #[test]
    fn test_server_types_accessible() {
        // Just verify the type path compiles
        let _: std::marker::PhantomData<server::DataTransferService> = std::marker::PhantomData;
    }

    #[test]
    fn test_error_result_reexported() {
        // Verify anyhow::Error and anyhow::Result are re-exported
        let _err: Error = anyhow::anyhow!("test error");
        let _res: Result<i32> = Ok(42);
    }

    #[test]
    fn test_transferpb_module_accessible() {
        let msg = transferpb::Message::default();
        assert_eq!(msg.id, 0);
    }
}
