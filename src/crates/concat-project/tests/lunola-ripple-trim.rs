// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regression of Lunola's adapter against the existing project engine.
// Compile the real UI command adapter against the real project engine, without
// recompiling the generated Slint UI on every RED/GREEN iteration.
#[path = "../../concat/src/ripple_trim.rs"]
mod ripple_trim;
#[path = "../../concat/src/timeline_av.rs"]
#[allow(dead_code)] // This harness exercises only the ripple-trim entry points.
mod timeline_av;
