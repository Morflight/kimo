//! The integration tests of kimo-cli, linked into one test binary (`all`) instead of one binary per file.
//! Each former `tests/<name>.rs` is the module `<name>`, so nextest names a test `<name>::<test>`.
//! Shared fixtures live in `common/` and are included by `#[path]` from the modules that use them.

mod commands;
