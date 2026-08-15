//! Structural goldens for book-length ePub extraction.
//!
//! Unlike `tests/golden.rs`, these compare a compact skeleton rather than full
//! text — see `tests/common/mod.rs` for why.

mod common;

use std::path::Path;

fn book(name: &str) {
    let path = Path::new("tests/golden/input/books").join(format!("{name}.epub"));
    let md = fte::epub::extract(name, &path).unwrap_or_else(|e| panic!("extracting {name}: {e}"));
    common::check_skeleton(name, &md);
}

#[test]
fn frankenstein_structure() {
    book("frankenstein-pg");
}

#[test]
fn tale_of_two_cities_structure() {
    book("tale-two-cities-pg");
}
