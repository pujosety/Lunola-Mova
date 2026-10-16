# Lunola Mova code signing — status and release gates

No certificate, signing-service approval or signed Lunola release is established
by this fork draft. Upstream Concat's SignPath application, approvers and funding
must not be represented as Lunola's. `AppPublisher=Lunola` is requested brand
metadata, **not evidence of a verified certificate subject or legal identity**.

## Before a signed release

- Establish an authorized distributor and certificate subject, credential custody,
  reviewer/approver policy, timestamp service and reproducible build provenance.
- Verify the exact commit, native dependency digests and corresponding source;
  pass application, clean-PATH staged smoke and installer lifecycle tests.
- Sign `lunola-mova.exe` with the authorized identity, stage that exact binary,
  compile the installers and sign the resulting setup EXE/MSI. Never alter a
  signed payload afterwards. Recompute final hashes after signing.
- Verify each actual signature, timestamp, chain and publisher with Windows SDK
  `signtool verify /pa /all /v <file>` and record the results. Inventory native
  DLL signatures separately; do not overwrite third-party attribution or claim
  that an application signature signs every dependency.
- Publish only with explicit approval after legal/source-access gates pass.

An unsigned development artifact must be labelled unsigned. Do not promise it
passes SmartScreen, Smart App Control or antivirus checks, or tell users to
disable security protections to install it. No installer has been built or
signature verified by this documentation task.

## Network and privacy verification

Inherited updater/model/API endpoints need a fork-specific audit. Do not reuse
upstream's absolute no-network/no-telemetry promise as measured Lunola behavior.
Before publication, document update and model destinations, user consent,
remote API bind/auth defaults, source offer for remote users where applicable,
logs and any telemetry after testing the exact build. Upstream download/support
sites are not official Lunola channels. See the
[publication gates](docs/lunola/packaging-publication.md).
