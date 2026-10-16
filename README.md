# Lunola Mova

**Make Every Frame Move.**

An independently branded development fork of **Concat**, created by **Jareer
(@jub0t) and Concat contributors**. Brand accent: `#B65CF2`. This checkout is a
draft, not an announced release or an upstream-endorsed product.

## Source capabilities, not verified feature promises

Inherited code includes a Rust/Slint editor, timeline, keyframes, effects/titles,
media import/export, transcription, text-to-speech, cutout, a CLI, JSON-RPC and feature-gated gRPC
interfaces. Each requires fork runtime checks before a release claim. MCP is
**unavailable in the upstream tree and deferred**, not a feature removed by this
fork. Do not infer performance, codecs, privacy or platform support from source
presence. One segmentation model is embedded; other model features can require
separate downloads whose endpoints and redistribution rights need review.

Windows x64 is the first verification target. The intended executable is
`lunola-mova.exe`; Rust package/library names remain `concat`. Draft Inno/WiX
sources live in `assets/windows/`, retaining their `concat.iss` / `concat.wxs`
filenames for integration. The [staging helper](scripts/lunola/stage-windows.py)
and [publication gates](docs/lunola/packaging-publication.md) do not constitute
a built installer. Windows ARM64, macOS, Linux, Android and iOS/iPadOS remain
fork-specific verification and publication work.

No Lunola download site, support channel, certificate or published installer is
confirmed here. Existing upstream-oriented workflows must be reviewed before
use; their outputs are not automatically Lunola releases.

## Documentation

- [Contributing](CONTRIBUTING.md) · [Signing status](CODE_SIGNING.md)
- [Inherited engine/build documentation](src/README.md)
- [Bahasa Indonesia](docs/README.id.md); other historical translation pages
  currently point to this canonical status pending fork translation review.

## Attribution, licensing and source access

[Source upstream: jub0t/Concat](https://github.com/jub0t/Concat) is **upstream**,
not official Lunola release/support. Retain upstream copyrights, authorship and
full Git history. Lunola Mova is not endorsed by Jareer or Concat contributors.

Read [LICENSE](LICENSE), [exceptions](LICENSE-EXCEPTIONS.md),
[upstream trademark provenance](TRADEMARK.md) and
[third-party notices](THIRD_PARTY_NOTICES.md). Never relabel the combined editor
as proprietary or replace upstream/third-party owners. Font, model, native and
Cargo license/notice materials must accompany the actual payload.

Binary distribution requires matching Corresponding Source, including fork
changes and relevant build/dependency materials. Staging includes a local
`SOURCE-README.md` and supplied source archive, not a fabricated source-offer
URL. An upstream link alone is not source access for the fork. Publication and
remote API operation need a reviewed source-access mechanism. Legal clearance,
artwork rights and complete notices remain unresolved release gates.
