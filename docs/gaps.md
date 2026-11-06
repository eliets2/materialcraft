# Gaps

> The assessment below predates the 2026-11-05 primitive rewrite. Its old library counts and
> compatibility claims are historical. See [atomic-workflow.md](atomic-workflow.md) and
> [ROADMAP.md](../ROADMAP.md) for the current scope and remaining validation.

Current integration also provides workspace-scoped `material_export`, real Height displacement
from the assigned material, explicit normal-map convention, and linear bitmap/input previews.
The native SBS and primitive-library limitations in [atomic-workflow.md](atomic-workflow.md) remain.

> **Last reviewed:** 2026-11-02 · **Last updated:** 2026-11-02 · **Change:** minor (portable `.mcmat` materials closed the published-archive gap) · **Target:** the professional node-based material workflow

Every known shortfall, most important first. Pick from the top.

| # | Gap | Evidence | Impact | Estimate | Parity doc |
|---:|---|---|---|---:|---|
| 1 | Node library breadth | 227 nodes vs. ~400 in the reference library | Recipes that need scan processing, more grunges, splines or SDF shapes | 30 h | target-app-parity (Node library) |
| 2 | GPU 3D view with tessellation and lights | CPU renderer with parallax only | Displacement previews, light setups | 12 h | target-app-parity (3D view) |
| 3 | Mesh-based generators and bakers | None | Workflows that start from a mesh | 15 h | target-app-parity (Node library) |
| 4 | Portable materials over MCP | `.mcmat` export runs from the app and the CLI; the MCP workspace sandbox refuses `file.*` commands and has no `material_export` tool | Sandboxed agents can't hand a material to TextureCraft | 1 h | target-app-parity (Files) |
| 5 | Function loops and reusable functions | Functions live on one parameter or processor | Iterative effects; sharing logic between nodes | 5 h | target-app-parity (Function graphs) |
| 6 | Input visibility conditions, per-instance output sizes | Not implemented | Organizing large graphs' inputs | 4 h | target-app-parity (Graph parameters) |
| 7 | `ui.screenshot` times out on this test machine | The window-capture callback never fires here; `view.screenshot` and the offscreen snapshot test work | Agents bridged to the app see views but not the whole window | 1 h | — |
| 8 | Translations and keymap editor | English only, fixed shortcuts | Non-English users; custom shortcuts | 8 h | localization-parity |
| 9 | Minimap and multi-graph tabs | One graph at a time, no overview | Navigating big graphs | 5 h | target-app-parity (Graph editing, UI and UX) |
| 10 | First signed release | Workflows copied, not run | Downloads | 3 h | target-app-parity (Platforms) |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-02 | minor | Portable `.mcmat` materials for TextureCraft closed gap 4; what's left is an MCP tool |
| 2026-11-02 | major | Closed the function editor, dynamic parameters, FX-Map/Text/SVG, presets and groups, browser file I/O and accessible names; re-ranked |
| 2026-11-02 | major | First list |
