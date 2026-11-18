# API editing agent protocol

`agent.rs` supplies the bring-your-own-key conversation loop over a `Tools` implementation. A user turn emits thinking/tool/result/reply events and makes at most 12 provider requests. Exhausting that budget returns an explicit stop message. The key is passed through the secret wrapper to the configured provider URL; the tests use only synthetic keys and loopback HTTP fixtures.

OpenAI requests use chat completions, named function calls and matching tool-result IDs. Anthropic requests use messages, a 2048-token response budget, tool-use blocks and grouped consecutive tool results. Plain user text is not merged into a tool-result array. Missing required response envelopes fail; missing optional call names/IDs or invalid OpenAI argument JSON follow the parser's empty/default values. A failed HTTP provider call emits an error and removes a dangling user message so retry does not duplicate it. Unreadable JSON and missing response envelopes follow the same failure cleanup and emit an error, so malformed-provider retries do not duplicate an unanswered user turn. Existing tool-result history is preserved.

History must be a readable, successful engine response with an `undo` array. Its last entry may be a string, a labelled object, a tool-labelled object or an unlabelled change. Only a successful tool call that increases known history depth creates a change card. Replacing an undo branch marks superseded cards undone. Undoing a card requests enough engine undo steps to include later active changes; missing cards, unavailable history, already-undone changes, transport errors and engine refusal fail without falsely marking cards undone.

Tool summaries list at most four argument entries and truncate long values on Unicode character boundaries. Emitted result previews are limited to 600 characters and conversation tool-result text to 4000, with an explicit truncation suffix.

Tests exercise actual HTTP serialization against local fixtures, response-envelope defaults/errors, Unicode clipping, history variants, failed undo preservation, branching, Anthropic grouping and the 12-step bound. These do not establish provider availability, real editing correctness or full application coverage; the feature remains unverified until the required platform reports and gates pass.

Card recording is separated from history lookup: an absent label falls back to the tool name, and a later-card undo leaves earlier active changes intact. Tests cover both engine-level error results and transport errors without creating successful change cards.
