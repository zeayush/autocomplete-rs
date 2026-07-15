//! Background writer that batches durability writes off the request path.
//!
//! The request path updates the in-memory trie synchronously and pushes
//! a `WriteOp` onto an mpsc channel. This task drains the channel,
//! coalesces N ops or T milliseconds worth, and issues one `WriteBatch`
//! per flush. That keeps p99 low without losing durability guarantees.

use crate::Result;
use tokio::sync::mpsc;
use std::sync::Arc;
use super::Storage;

pub enum WriteOp {
    Upsert { tenant: String, term: String, frequency: u64 },
    Delete { tenant: String, term: String },
}

/// Handle held by the engine — clone freely, it's just a channel sender.
#[derive(Clone)]
pub struct WriterHandle {
    // tx: mpsc::Sender<WriteOp>,
}

impl WriterHandle {
    /// Spawn the background flush task and return its handle.
    ///
    /// HINT: parameters worth exposing as config:
    ///   - `channel_capacity` (default 8192)
    ///   - `max_batch_size`   (default 1000)
    ///   - `flush_interval`   (default 50ms)
    pub fn spawn(storage: Arc<Storage>) -> Self {
        // HINT sketch:
        //   let (tx, mut rx) = mpsc::channel(8192);
        //   tokio::spawn(async move {
        //       let mut buf = Vec::with_capacity(1000);
        //       let mut ticker = tokio::time::interval(Duration::from_millis(50));
        //       loop {
        //           tokio::select! {
        //               op = rx.recv() => match op { Some(o) => { buf.push(o); if buf.len() >= 1000 { flush(&storage, &mut buf); } }, None => break, }
        //               _ = ticker.tick() => if !buf.is_empty() { flush(&storage, &mut buf); }
        //           }
        //       }
        //   });
        //   Self { tx }
        let _ = storage;
        todo!()
    }

    /// Non-blocking send. If the channel is full, this is where you decide
    /// whether to drop, block, or return an error.
    pub async fn submit(&self, op: WriteOp) -> Result<()> {
        // HINT: `.send(op).await.map_err(...)`. For back-pressure signaling,
        // consider `try_send` and returning an error the API can surface as 503.
        let _ = op;
        todo!()
    }
}
