// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regression of Lunola's adapter against the existing project engine.
// Compile the real UI command adapter against the real project engine, without
// recompiling the generated Slint UI on every RED/GREEN iteration.
#[path = "../../concat/src/timeline_av.rs"]
mod timeline_av;
#[path = "../../concat/src/ripple_trim.rs"]
mod ripple_trim;
