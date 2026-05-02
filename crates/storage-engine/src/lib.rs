// Copyright (c) 2023 - 2026 Restate Software, Inc., Restate GmbH.
// All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Backend-agnostic storage engine traits.
//!
//! Restate today has four direct RocksDB consumers — log-server, bifrost's
//! local_loglet, metadata-server's raft storage, and partition-store. Each
//! couples to `rocksdb::*` types directly for write batches, iterators,
//! merge operators, and option configuration. This crate is the seam that
//! lets those consumers be ported off RocksDB onto an alternative engine
//! (fjall is the immediate target) without rewriting the consumer logic.
//!
//! # Status
//!
//! Stub. The trait surface is intentionally empty — it grows organically as
//! the first consumer (log-server) is migrated to consume it. Adding
//! traits speculatively here would be guesswork; adding them in lockstep
//! with consumer migration produces the right shape on the first try.
//!
//! # Sequence
//!
//! 1. **log-server migration** drives the initial trait surface: write
//!    batches with merge support, prefix/range iterators, batched
//!    multi-get, and the lifecycle hooks (open, flush, sync) the
//!    `RocksDbLogStoreBuilder` already exposes.
//! 2. **bifrost local_loglet** validates the surface holds under hot-path
//!    write pressure.
//! 3. **metadata-server raft** validates a third independent consumer
//!    (no merge operator, two CFs).
//! 4. **partition-store** is gated on a snapshot/checkpoint redesign —
//!    `Checkpoint::export_column_family` has no fjall analog.

#![deny(missing_docs)]

pub mod error;

pub use error::{Error, Result};
