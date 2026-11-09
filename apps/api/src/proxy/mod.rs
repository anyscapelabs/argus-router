pub mod adapters;
pub mod ir;
pub mod service;
pub mod sse;

pub use ir::{parse_json_lenient, ErrCode, Errx, Quirks};
