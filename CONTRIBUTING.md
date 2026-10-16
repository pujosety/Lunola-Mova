# Contributing to Lunola Mova

Coordinate changes with the fork maintainer through the channel actually supplied
with your checkout. No Lunola tracker, Discord or published build is established
here; upstream does not provide official support for this fork.

## Build and verification

The workspace is `src/`. Rust package/library names remain `concat`; after
product-identity integration the editor is `lunola-mova.exe`. Read the inherited
[src/README.md](src/README.md) and pinned `src/rust-toolchain.toml`, `Cargo.lock`
and native dependency lock before building. Native FFmpeg, sherpa, ONNX Runtime,
CMake, C++ compiler/SDK and libclang must match the target/link configuration.
Do not commit workstation-specific dependency paths.

After prerequisites are ready, from `src/`:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --profile app -p concat --target x86_64-pc-windows-msvc --locked
```

These are requested checks, not recorded passes. Follow the actual CI
nextest/doctest/gRPC/export and portability checks too. From repository root,
run `python scripts/models.py --check` for model-table changes and
`python scripts/locales.py` for UI string inventory changes. See
[TRANSLATING.md](TRANSLATING.md). Do not publish models or invoke release
workflows without reviewed rights and explicit authorization.

Smoke the real application and clean-PATH staged DLL bundle; verify
create/import/edit/save/reopen/export. Test inference/model downloads separately.
Record exact commands, versions, exit codes, skips and evidence. Installer XML
or staging inventory checks are not application tests. JSON-RPC/gRPC need their
own tests; MCP is absent upstream and deferred, not an inherited working feature.
Internal `CONCAT_*` compatibility names may remain pending deliberate migration.

## License and authorship

Retain existing SPDX headers, **2026 Jareer and Concat contributors**, third-party
copyrights and full Git history. Add your own copyright for new work, never
replace another author's line. New project source uses `AGPL-3.0-or-later` unless
a reviewed, genuinely separate component has another compatible license.
Read [LICENSE](LICENSE), [exceptions](LICENSE-EXCEPTIONS.md),
[third-party notices](THIRD_PARTY_NOTICES.md) and [TRADEMARK.md](TRADEMARK.md).

[CLA.md](CLA.md) remains upstream provenance. It names upstream and must not be
assumed to grant the Lunola maintainer equivalent rights. A sign-off does not
automatically extend that CLA. Fork contribution/exception policy requires
explicit review before accepting external contributions. Attribution transfers
neither ownership nor trademark rights.

Document each imported dependency, artwork, font and model with origin, version,
changes, actual full license/notices and redistribution conditions. Follow the
[platform/legal packaging gates](docs/lunola/packaging-publication.md).
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) is retained; its upstream enforcement
contacts are not automatically designated Lunola contacts.
