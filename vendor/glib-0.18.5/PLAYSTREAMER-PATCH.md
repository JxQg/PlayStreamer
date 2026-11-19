# PlayStreamer glib security backport

This directory contains the unmodified crates.io glib 0.18.5 source package, except
for the two-line security fix described below and this provenance file. The Cargo
cache marker `.cargo-ok` is omitted. The package retains its real version, 0.18.5.

- Upstream: https://github.com/gtk-rs/gtk-rs-core
- Source commit: `42b9caf98e03ded086362d9653ca58fe94dc8658` (subdirectory `glib`)
- Crates.io package SHA256: `233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`
- License and copyright: see `LICENSE` and `COPYRIGHT` in this directory.

`src/variant_iter.rs` applies upstream commit
[`b5a4071e439bef2b5eea76c3aa25e5ae84839e34`](https://github.com/gtk-rs/gtk-rs-core/pull/1343):
the pointer in `VariantStrIter::impl_get` is declared mutable and passed to
`g_variant_get_child` as `&mut p`. This fixes GHSA-wrw7-89jp-8q8g /
RUSTSEC-2024-0429 without changing the glib 0.18 API required by the GTK dependencies.

The root Cargo patch selects this source for the desktop dependency graph.
`tools/glib-security` runs the upstream variant iterator tests against this exact
path dependency using a separate lockfile and an optimized Linux build. Run:

```sh
cargo test --manifest-path tools/glib-security/Cargo.toml --release --locked --lib
```

The upstream advisory currently recognizes releases >=0.20.0 as patched; this
backport retains 0.18.5 and must not be described as an upstream 0.20 release.
Remove this override when the desktop dependencies support a patched upstream
glib release, and remove the separate regression project at the same time.
