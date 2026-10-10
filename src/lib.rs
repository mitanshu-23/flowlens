//! Static analysis of Rust source: index a file's definitions and references,
//! then read or draw how they connect.
//!
//! The main use is [`review`]: compare the code with an earlier version and
//! show, feature by feature, what it did before and what it does now.
//!
//! Underneath, [`index::index_file`] returns a [`index::FileIndex`] you can
//! query for callers and callees, and [`workspace::Workspace`] links a whole
//! crate's files together.

pub mod cfg;
pub mod dot;
pub mod html;
pub mod index;
pub mod json;
pub mod model;
pub mod modmap;
pub mod provenance;
pub mod report;
pub mod review;
pub mod workspace;
