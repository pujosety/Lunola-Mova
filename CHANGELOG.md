# Changelog

Changes are grouped by release. This file is a local development ledger and does not imply publication or authorship of upstream Concat changes.

## Unreleased — Lunola Mova fork work

- **In progress:** Lunola product identity and platform packaging changes; preserve upstream engine/API/project identity and required provenance.
- **In progress:** Cutout controls, mask/render integration and local inference verification; quality remains not visually assessed.
- **In progress:** A/V lane/link and timeline-cursor work; tests/build evidence is snapshot-specific.
- **In progress:** Alpha-preserving PNG sequence export alongside the existing Movie default. Engine integration tests passed 5/5; UI wiring type-check and a full Windows app build/version smoke passed before the latest locale JSON additions. Actual GUI export remains NOT_RUN.
- Added localized Movie/transparent PNG-sequence export labels to all 24 non-English locales; locale inventory reports 0 missing/0 stale keys. Native-speaker wording review remains advisable.
- **Implemented, focused-test verified:** Avoid sharing a Vulkan compositor queue with worker threads for export and effect-card generation; workers fall back to a compositor they create themselves. Full-app runtime behavior is not yet verified.
- Added fork audit/control-plane documents in `docs/` for architecture, dependencies, platform support, technical debt, issue triage, fix order, test matrix, performance baseline and feature roadmap.

Unreleased entries describe work in a dirty working tree. Confirm current source and test evidence before moving an item to a release section.

## 0.2.7 (upstream baseline)

The repository's recorded upstream baseline package version is 0.2.7. Consult upstream release history and preserved attribution for upstream release details; this changelog does not reattribute upstream work to Lunola Mova.
