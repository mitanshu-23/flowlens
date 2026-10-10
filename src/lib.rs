//! Static analysis of Rust source that shows how the code flows: index a
//! crate's definitions and references, then read or draw how they connect.
//!
//! The main output is [`review`]'s page: a map of every feature, what it
//! calls in order, and the steps each function takes, in plain words. With
//! git history the same page also compares the code with an earlier version,
//! feature by feature: what it did before and what it does now.
//!
//! Underneath, [`index::index_file`] returns a [`index::FileIndex`] you can
//! query for callers and callees, and [`workspace::Workspace`] links a whole
//! crate's files together.

pub mod index;
pub mod json;
pub mod model;
pub mod modmap;
pub mod provenance;
pub mod review;
pub mod workspace;
