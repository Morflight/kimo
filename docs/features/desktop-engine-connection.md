# Desktop engine connection

The desktop finds its engine in this order: an existing KIMO_MCP_BIN override, beside the executable, its bin directory, the deb-style ../lib/kimo directory, resource directories, nearby workspace target profiles, then PATH. Discovery only checks file presence for explicit locations and platform executable rules for PATH. It does not prove that a discovered binary can launch.

RPC launches a child with piped stdin/stdout and a file or discarded stderr, exchanges newline-delimited JSON-RPC, ignores non-JSON lines and notifications, routes responses by numeric request ID, and reports transport, engine and deadline failures. Handshake initializes the connection and lists tools. A fresh probe disables spectator activity and stops its child after handshake. The supervisor reports missing, stopped, running or crashed status; a tool call can restart a dead connection. Restarting does not restore the old engine's project.

Tests check discovery precedence inside isolated child processes, platform binary names, missing binaries, malformed result blocks, missing probe metadata and missing/stopped/crashed supervisor states. Unix fixture tests check handshake, tool calls and restart after a child exits. Default metadata uses `?`; tools without valid action counts count as one action. Result text joins text blocks and labels other block types.

Complete RPC fault coverage, native Windows subprocess parity and the real-engine probe remain required. The real-engine probe now fails if KIMO_MCP_BIN is absent. Desktop coverage CI builds the actual workspace kimo-mcp binary and supplies its path before testing. This feature remains unverified.

Additional subprocess fault tests use an isolated Python fixture on stdin/stdout. They assert exact engine error messages and defaults, ignore unrelated IDs/notifications and malformed JSON, reject invalid UTF-8, clear pending requests after timeout or broken input, tolerate a failed stderr-log destination, and preserve handshake failures. A readiness file synchronizes the closed-input case so it does not depend on a guessed process-start delay.

Child attachment is a private boundary used by production spawn. If either required pipe is absent it kills and waits for that child before returning the precise error. Tests provide real children with missing pipes and verify prompt rejection. Another private lookup checks the absent-connection error possible when stop races with a successful start; its boundary test does not claim to reproduce a particular thread schedule.

A portable real-engine failure test supplies an invalid memory-budget value only to its child process. It requires a crashed supervisor, no retained connection, an explicit transport-closure error and the actual engine's configuration diagnostic in its isolated stderr log. This complements the fixture's rejected handshake response without changing the host's configuration.
