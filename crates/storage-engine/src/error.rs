// Copyright (c) 2023 - 2026 Restate Software, Inc., Restate GmbH.
// All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Engine-agnostic error type.
//!
//! Backend implementations wrap their concrete errors (`rocksdb::Error`,
//! `fjall::Error`) into `Error::Backend` and let callers pattern-match on
//! the engine-level variants for behavior they actually want to discriminate
//! on (e.g. merge operator mismatch, which both backends surface).

use std::fmt;

/// Errors produced by a storage-engine backend.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Backend-specific failure with no engine-level discriminant. Wraps
    /// the underlying error type-erased so consumers don't need to depend
    /// on `rocksdb` or `fjall` directly.
    #[error("backend error: {0}")]
    Backend(Box<dyn std::error::Error + Send + Sync + 'static>),

    /// Reopened a keyspace whose persisted merge operator name doesn't
    /// match the one supplied at open time. Reading or compacting in this
    /// state would silently apply incompatible semantics to existing
    /// merge operands, so backends fail open with this error instead.
    #[error(
        "merge operator mismatch on open: persisted {stored:?}, supplied {supplied:?}"
    )]
    MergeOperatorMismatch {
        /// Operator name persisted in the keyspace's metadata.
        stored: String,

        /// Operator name supplied this open, or `None` if no operator was
        /// supplied for a keyspace that previously had one.
        supplied: Option<String>,
    },
}

impl Error {
    /// Wrap a backend-specific error into the engine-agnostic `Backend`
    /// variant.
    pub fn backend<E: std::error::Error + Send + Sync + 'static>(err: E) -> Self {
        Self::Backend(Box::new(err))
    }

    /// Convenience for backends emitting a static-string failure.
    pub fn backend_msg(msg: impl fmt::Display) -> Self {
        Self::Backend(msg.to_string().into())
    }
}

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;
