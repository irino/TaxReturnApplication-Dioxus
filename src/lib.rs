//! Library surface for pure logic and persistence.
//! UI lives in the binary (`main.rs`) and is kept out of this crate root so
//! core/storage stay testable without launching a desktop window.

pub mod core;
pub mod pdf;
pub mod storage;
