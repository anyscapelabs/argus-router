pub mod adapters;
pub mod ir;
pub mod service;
pub mod sse;

pub use ir::{ErrCode, Errx, Quirks, parse_json_lenient};
