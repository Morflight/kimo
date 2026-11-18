# Spectator result formatting

The pure helpers in `reads.js` turn read results and selected mutating-tool arguments into compact visible rows. Read cards hold at most four rows; generic values prioritize scalars, then lists, then maps. Metadata fields, internal identifiers, long prose and empty/unrepresentable values are omitted. Long paths display the filename, and long text is truncated. Numeric values retain unit conventions, including bytes and normalized opacity percentages. These are presentation strings, not engine values written back to a project.

Known read tools have measurement-specific rows: scopes distinguish luma, RGB, clipping and crushing; caption validation distinguishes profile/group counts from legibility measurements. Unknown read tools use generic rows. Mutating-call rows can prepend a known clip name and mark emphasis; text path/style/weight and animator chips describe the requested operation. Color summaries distinguish RGB gains, key colors, luminance comparisons and Lab comparisons instead of interchanging those measurements.

`read_format_test.mjs` loads the complete module and its real cyclic router imports. It checks scalar/array/map boundaries, non-finite values, internal-ID/path filtering, text truncation in existing helpers, row ordering/caps, threshold labels, scope/caption units, mutation emphasis and color-space labels. The browser row proofs remain required for actual layout and visual interpretation. Missing optional results omit rows rather than inventing a successful measurement.

The formatter remains legacy-unverified: complete coverage and all malformed-response boundaries still require qualification.

Additional exact-value cases cover project counts, cache memory units and hit rates, environment diagnostics, history, markers, edit points, HLG display values and color-pipeline field aliases.
