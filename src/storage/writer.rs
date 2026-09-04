//! Background writer that batches durability writes off the request path.
//!
//! The request path updates the in-memory trie synchronously and pushes
//! a `WriteOp` onto an mpsc channel. This task drains the channel,
//! coalesces N ops or T milliseconds worth, and issues one `WriteBatch`
//! per flush. That keeps p99 low without losing durability guarantees.

use super::Storage;
use crate::{Error, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

pub enum WriteOp {
    Upsert { tenant: String, term: String, frequency: u64 },
    Delete { tenant: String, term: String },
}

/// Tunables for the flush loop.
#[derive(Debug, Clone, Copy)]
pub struct WriterConfig {
    pub channel_capacity: usize,
    pub max_batch_size: usize,
    pub flush_interval: Duration,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 8192,
            max_batch_size: 1000,
            flush_interval: Duration::from_millis(50),
        }
    }
}

/// Handle held by the engine — clone freely, it's just a channel sender.
#[derive(Clone)]
pub struct WriterHandle {
    tx: mpsc::Sender<WriteOp>,
    /// Shared so `WriterHandle` stays `Clone`; only `join` ever takes it.
    task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl WriterHandle {
    /// Spawn the background flush task with the default tunables.
    pub fn spawn(storage: Arc<Storage>) -> Self {
        Self::spawn_with(storage, WriterConfig::default())
    }

    pub fn spawn_with(storage: Arc<Storage>, config: WriterConfig) -> Self {
        let (tx, mut rx) = mpsc::channel::<WriteOp>(config.channel_capacity);

        let task = tokio::spawn(async move {
            let mut buffer: Vec<WriteOp> = Vec::with_capacity(config.max_batch_size);
            let mut ticker = tokio::time::interval(config.flush_interval);
            // A missed tick means we were busy flushing, not that we owe an
            // extra flush; skipping avoids a burst of empty wake-ups.
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    received = rx.recv() => match received {
                        Some(op) => {
                            buffer.push(op);
                            if buffer.len() >= config.max_batch_size {
                                flush(&storage, &mut buffer);
                            }
                        }
                        // Every sender is gone: drain what is left and stop.
                        None => break,
                    },
                    _ = ticker.tick() => flush(&storage, &mut buffer),
                }
            }

            flush(&storage, &mut buffer);
            if let Err(err) = storage.flush() {
                tracing::warn!(error = %err, "final rocksdb flush failed");
            }
        });

        Self { tx, task: Arc::new(Mutex::new(Some(task))) }
    }

    /// Queue a write. Applies back-pressure by awaiting channel capacity;
    /// the request is already durable-in-memory by this point, so waiting
    /// here throttles writers rather than losing data.
    pub async fn submit(&self, op: WriteOp) -> Result<()> {
        self.tx.send(op).await.map_err(|_| Error::Backpressure)
    }

    /// Queue a write without waiting. Returns [`Error::Backpressure`] when the
    /// channel is full, which the API surfaces as 503.
    pub fn try_submit(&self, op: WriteOp) -> Result<()> {
        self.tx.try_send(op).map_err(|_| Error::Backpressure)
    }

    /// Close the channel and wait for the task to drain and flush. Consumes
    /// the handle so the sender is dropped, which is what stops the loop.
    pub async fn join(self) -> Result<()> {
        let WriterHandle { tx, task } = self;
        drop(tx);
        let handle = task.lock().await.take();
        if let Some(handle) = handle {
            if let Err(err) = handle.await {
                tracing::warn!(error = %err, "writer task did not exit cleanly");
            }
        }
        Ok(())
    }
}

/// Write `buffer` as one RocksDB batch and clear it.
///
/// A failed batch is logged rather than propagated: the task has no caller to
/// return to, and dropping the ops keeps the loop alive for later writes. The
/// in-memory trie remains authoritative until the next restart.
fn flush(storage: &Storage, buffer: &mut Vec<WriteOp>) {
    if buffer.is_empty() {
        return;
    }

    let ops: Vec<(&str, &str, Option<u64>)> = buffer
        .iter()
        .map(|op| match op {
            WriteOp::Upsert { tenant, term, frequency } => {
                (tenant.as_str(), term.as_str(), Some(*frequency))
            }
            WriteOp::Delete { tenant, term } => (tenant.as_str(), term.as_str(), None),
        })
        .collect();

    if let Err(err) = storage.write_batch(&ops) {
        tracing::error!(error = %err, batch = ops.len(), "batched write failed");
    } else {
        tracing::debug!(batch = ops.len(), "flushed write batch");
    }

    buffer.clear();
}
