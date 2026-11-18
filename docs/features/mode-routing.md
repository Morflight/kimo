# Spectator workspace selection

The spectator defines sixteen workspaces, each with panel kinds and positive-area design rectangles. Its common Agent panel defaults can be overridden per workspace; the mixing workspace uses the shared default row height of 37 design pixels. These definitions do not themselves render or connect to an engine.

For engines without an explicit mode, `inferMode` selects the first matching tool-name rule. Precedence matters: render_scene3d_sequence goes to Export, set_camera_keyframe to Scene 3D, timeline track operations to Edit, track mattes to Roto, graph animation operations to Graph and color-pipeline operations to Color. Ordinary grading operations select Grading. All sixteen workspace families are exercised by the routing table test.

Missing/empty and unknown names return no inferred workspace. Read-only names are recognized case-insensitively and never request a workspace change, even if they contain an editing family word. Mutating tool rules retain their existing case-sensitive matching. Explicit mode handling belongs to the reducer and is not overridden by this helper.

`modes_test.mjs` loads the complete unchanged module, checks routing and precedence, read-only/unknown inputs, panel renderer correspondence, rectangle shape/finite coordinates/positive area and common defaults. Browser rendering, responsiveness and interaction remain separate acceptance requirements.
