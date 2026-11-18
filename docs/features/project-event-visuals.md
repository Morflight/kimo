# Project event visuals

The project-event module presents color-pipeline, project/history/template/cache and QA tool responses in existing editor panels. It consumes start/completion/error events, retains start arguments until completion and uses human-readable names. Presentation does not change engine state.

Pipeline rows distinguish input, working and output spaces, clip overrides and HDR metadata. Numeric gamut/transfer identifiers map to named spaces; absent or unknown components are omitted. Disabled processing is explicitly display-referred. Refused pipeline mutations leave existing rows unchanged, except an OCIO delivery failure displays the refused view and marks it as an error. A later successful view selection clears that error. Clearing HDR metadata displays cleared values only when metadata existed.

History association fills the pending placeholder created by the shared undo mechanism; it does not overwrite an already-associated step. Loading a project creates placeholders for unseen history, caps those placeholders at 47 and appends the visible load operation. Snapshots and restoration maintain timeline/pipeline/panel state. Project and QA browser proofs remain required for actual displayed results and layout.

The focused Node tests load complete cyclic modules through the same router entry order as the UI. They assert named color spaces, missing values, refusal preservation, clip overrides, HDR clearing, OCIO recovery and history bounds. The browser C suite covers project, history, template, cache and QA events. These tests do not yet qualify every malformed event, every rendering branch or the underlying native engine; the feature remains unverified.
