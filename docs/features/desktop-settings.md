# Desktop non-secret settings

`Settings` persists preferences as JSON. Missing, unreadable, malformed or wrongly typed files load defaults without overwriting the original. Partial older objects inherit defaults: MCP off, OpenAI provider, Claude Code agent, update checks on, empty overrides/history and incomplete onboarding/setup. Unknown fields are ignored by serde.

`touch_recent` moves an exact path to the front, removes duplicates and retains at most eight entries. Paths and model labels preserve Unicode. Settings contain provider identifiers and remembered-provider names, never the secret API keys themselves; the separate credential store owns secrets.

`save` creates parent directories, writes pretty JSON to the sibling `.json.tmp` path and renames it over the destination. Parent creation, serialization, temporary write and rename errors are returned as strings. Failed temporary writes preserve the prior destination; failed renames may leave a temporary file. This is replacement semantics, not a promise of fsync durability or concurrent-writer coordination.

Unit tests check default/partial loading, complete Unicode round trips, recent ordering/capping, invalid input preservation and deterministic parent/write/rename failures with isolated temporary directories. They do not qualify an OS credential store. Native platform coverage remains required and this feature remains unverified until all counters and checks pass.

Serialization failure is exercised through a private encoder dependency used by the same write/rename implementation. Production still supplies `serde_json::to_vec_pretty`; the fault test supplies an explicit serialization error and verifies that neither the destination nor a temporary file is replaced. The current string/boolean settings schema does not normally trigger a JSON serialization error; this test validates its error-handling boundary without pretending it did. An empty destination is also rejected.
