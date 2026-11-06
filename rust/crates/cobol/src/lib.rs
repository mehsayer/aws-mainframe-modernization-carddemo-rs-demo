//! COBOL data semantics for the CardDemo Rust port.
pub mod copybooks;
pub mod ebcdic;
pub mod edit;
pub mod field;
pub mod numeric;
pub mod text;

pub use numeric::{Dec, Pic, Usage};
