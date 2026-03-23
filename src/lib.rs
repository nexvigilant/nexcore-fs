//! Zero-dependency filesystem utilities for the NexCore ecosystem.
//!
//! Replaces `dirs`, `walkdir`, `glob`, `tempfile`, and `shellexpand` crates
//! with NexVigilant-owned implementations.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

pub mod dirs;
pub mod glob;
pub mod walk;
