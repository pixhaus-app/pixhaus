//! Pixhaus import/export and the on-disk project format.
//!
//! `io` owns reading and writing: the `.pixhaus` project format, PNG, sprite
//! sheets, and the importer/exporter traits other crates register against. It
//! depends only on `core` and never on the UI.
//!
//! Scaffold stage: a stub. The format and codecs land per architecture bible
//! sections 18 and 19.
