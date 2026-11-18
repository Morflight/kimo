# MCP client configuration

A launch description contains a command, ordered arguments and ordered environment pairs. The module generates a stdio MCP server object, client registration arguments, copyable Claude/Codex commands and a Codex TOML table. Empty environment lists omit the environment field; argument lists remain present even when empty. The typed Claude registration object explicitly declares stdio. Generating snippets does not launch or register a process.

POSIX single quotes protect spaces and embedded apostrophes. The Windows formatter doubles embedded double quotes and quotes words containing spaces; these formatting tests do not establish compatibility with every Windows shell. TOML strings escape backslashes, quotes, newline, tab and other ASCII controls; keys outside the bare-key character set are quoted. Unicode values are preserved.

Tests assert exact command ordering, Windows paths, JSON round trips, empty values, control characters, non-bare keys, and consistency of the complete serialized snippet bundle. This contract remains unverified pending complete platform qualification; it does not certify installed third-party clients or their evolving command-line interfaces.
