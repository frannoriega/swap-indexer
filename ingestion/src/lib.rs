//! Ingestion library surface.
//!
//! The binary (`main.rs`) wires the Carbon pipeline; the reusable pieces live
//! here so tests and benchmarks can exercise them directly.

pub mod mapping;
pub mod processor;
