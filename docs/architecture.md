# Architecture

> **Last reviewed:** 2026-11-05 · **Last updated:** 2026-11-05 · **Change:** evaluator and editor organization · **Target:** the professional node-based material workflow

MaterialCraft is a Cargo workspace of small crates in strict layers (`cargo xtask layers` enforces
them). Everything below the UI is plain Rust with no UI toolkit and builds for
`wasm32-unknown-unknown`.

```text
L0  core      document model: Package → Graph → Node / Connection / Frame / Comment / Pin / ExposedParameter;
              Value (float, vectors, int, bool, color, enum, text, curve, gradient, function graph)
L0  image     Image: grayscale or RGBA f32 planes behind an Arc (copy-on-write, free to share);
              wrap/clamp sampling, bilinear, box/bilinear resampling, sRGB, 8-bit display
L0  geom      meshes, primitives (rounded cube, sphere, cylinder, plane, torus, material ball), BVH, camera
L1  nodes     30 main-graph entries (NodeDefinition), 85 function operations, four pattern node kinds;
              shared image operations and iterative function/pattern execution
L2  eval      cooking: topological order, output sizes (relative to input / parent, absolute, preview
              cap), parameter resolution (defaults, overrides, bindings to exposed inputs), the
              content-addressed LRU cache, graph instances (sub-graphs, recursion-guarded)
L2  formats   native .sbs packages, .mcmat portable materials, bitmap/SVG decode,
              texture encode (PNG/JPEG/TGA/TIFF/BMP/WEBP/EXR/HDR), atomic writes
L3  engine    Session (package, open graph, selection, 2D/3D view state, snapshot undo, cache),
              the command registry, inspect, sample materials and graph templates
L3  render    the 3D view: BVH ray casts, the height output displacing a subdivided mesh, GGX PBR with
              prefiltered IBL + SH diffuse, alpha transparency (layered ray compositing) and an
              alpha channel view, ACES tone mapping; procedural environments
L4  automation MCP server (rmcp), JSON-lines RPC (stdio/TCP with token), bridge client, workspace sandbox
L5  ui-egui   the egui shell: docking (craft-ui), graph editor, library, properties, 2D/3D views,
              dialogs, menus, control channel with synthetic input
apps          materialcraft (desktop), materialcraft-cli, materialcraft-web
```

## Cooking

A node's cook is a pure function `fn(&CookContext) -> Result<Vec<Image>, String>`: `CookContext` carries the output
size, the seed, tiling, the inputs (by port order, `None` when unconnected), the resolved parameters,
a resource decoder and the graph's exposed values. `eval::cook` walks the graph in dependency order.
For each node it hashes the definition id, resolved parameters, size, seed, tiling, the keys of its
input images, resource bytes and (for instances) the sub-graph: equal keys hit the cache, so an edit
re-cooks only the nodes downstream of it, and undo finds earlier results again. A failing node shows
a magenta image and its error; nodes after it still cook.

Each cook builds a borrowed `core::GraphIndex` for node and connection lookups. `PreparedGraph`
resolves the needed nodes' ports once; `graph_execution` schedules them and `node_execution` handles
a single node. Parameter resolution, graph instances, value execution, output collection and cache
keys have separate modules. The index lives only for the cook, so it needs no invalidation protocol.

Function execution shares one interpreter. Pixel Processor borrows the caller's variables and keeps
only assignments in its per-pixel state; ordinary function evaluation retains the full environment
for callers such as FX-Map that read the resulting variables.

Sizes: generators take the graph ("parent") size; filters take their main input's size; both can be
offset by powers of two or set absolutely. While editing, outputs are capped at the preview size
(512 px by default), with pixel-based parameters scaled by `w / 256` so previews and full-size
exports look the same. Exports cook at full size through the same cache.

The 2D toolbar and the 3D view's Display menu expose the shared preview limit (256, 512, 1024,
2048 or 4096 px, or Full size). This is a cap, so a 4K preview requires a graph/node configured
for 4096 px or larger. Both controls run `view.previewSize` and persist the choice in preferences;
control-channel calls to the same command also retain their size across UI frames. The 3D view
uses those cooked maps, while its rendered frame follows the panel's physical pixel dimensions.
If the device supports smaller textures than the cooked image, the 2D view reduces only the
uploaded texture to that limit; source pixels, readout and screenshots retain the cooked size.

The desktop UI cooks on a worker thread (`ui-egui/src/cook.rs`) from a snapshot of the package and
keeps drawing the last results; the browser cooks on the UI thread.

## Commands and automation

Every user-visible action is a `CommandSpec` (`engine/src/*_cmds.rs`): id, label, menu, shortcut,
params doc, `enabled`, `run`. The UI calls them through `MaterialCraftApp::run`; the CLI, the JSON
control channel and MCP dispatch the same ids. `Session::edit` snapshots the package before running
an edit and restores it on error; `edit_merged` folds repeated edits with the same key (a slider
drag) into one undo step. See [control-protocol.md](control-protocol.md).

## The UI

`MaterialCraftApp` owns the session and `UiState` (serde, persisted by eframe, readable and writable
through `ui.inspect` / `ui.set`). The dock (craft-ui) hosts eight panels around the protected graph
panel. The graph editor lays nodes out in graph space, draws cards with thumbnails from the cook
results (uploaded once per cache key), wires as cubic beziers between typed ports (hit-tested
against the curve, so wires can be selected, deleted, swapped or split with a reroute dot), and turns
gestures into commands only when they complete. See [wires-and-alpha.md](wires-and-alpha.md). The 3D view renders with `materialcraft-render`
on the CPU: half resolution while the camera moves, 2×2 supersampled when it rests.

The 3D material has its own `MaterialPreview` assignment state. Automatic graph output usages and
explicit `MaterialSource` overrides resolve to channels from `core::MaterialChannel`, shared with the
renderer and menus. A source records its graph, node and named port; selection and 2D preview do not
change it. `PreviewCook` publishes the editor cook and any other assigned graphs together through
the existing worker/cache, so opening a Function Graph preserves the material. Render invalidation
includes both source assignments and cooked content. Snapshot undo and graph deletion retain or
adjust source ownership; package replacement clears old assignments.

`core::ComponentUsage` defines output metadata once for node properties, SBS and exports. Its
indices stay stable as choices are added; renderable channels remain a separate capability.
Explorer and canvas node labels share display formatting without rewriting identifiers. Explorer
lists every graph input/output, and graph drops dispatch the existing material assignment command.

`crates/ui-egui/tests/snapshot.rs` renders the real UI offscreen with egui_kittest + wgpu for visual
checks and README screenshots.

The regular and function canvases separate drawing, geometry, navigation and input handling.
Shared modules own node cards, selection and annotation gestures. The regular canvas borrows the
document while drawing, then dispatches edits. Graph Properties follows the same pattern; its
`property/` modules separate panel routing, node and graph settings, parameter controls, curves
and gradients. Function and pattern Properties dispatch value edits without cloning graphs in the UI.

The properties panel builds every parameter editor from the node catalog; the Levels node adds a
histogram editor with range handles on top of its sliders ([levels-editor.md](levels-editor.md)).

### Shared graph editing

The graph panel renders one toolbar before dispatching to the material, Function Graph or FX-Map
canvas. Framing, alignment, distribution and snapping occupy the same positions. Image inserts,
parent texture size, annotations and parent-graph navigation appear only where the graph supports
them. Alignment uses the same engine command for all three models, with explicit embedded node IDs
and an explicit Function target or FX-Map owner ID, so overlapping IDs cannot move the parent material nodes.
Pixel Processor, Value Processor and parameter functions use that same main panel and function node
catalog, with one target-aware availability rule shared by node creation, Library and Add node.
Value Processor omits the two sampler entries; saved functions retain their existing operations.
A breadcrumb and parent action return from nested functions, FX-Maps and local graph instances
while retaining the parent view and material preview.

`ui-egui/graph/graph_camera.rs` owns framing, pointer-centered scroll/pinch zoom and navigation
shortcuts. `node_selection.rs` owns click and box selection; `port_hit.rs` owns nearest-port picking;
`wire_geometry.rs` owns curve construction. Node movement uses the engine's `graph_grid::translate`
for both previews and commits.

`engine/graph_edit/` owns Function/FX-Map selection commands, clipboard remapping, deletion and
connected-node traversal. Its small model adapters supply node fields, link direction, ID allocation
and result/root handling. Material clipboard commands retain their named-port connections and
cross-graph clipboard rules. Rendering and connection validation remain specific to each model;
all document changes still pass through engine commands and snapshot undo. `selection_commands.rs`
routes the existing selection and clipboard command IDs to these adapters, including headless calls.
Read-only selection actions borrow the graph. Document, selection and clipboard updates are published
only after validation succeeds; clipboard provenance prevents external links crossing graph targets.
`graph_validation.rs` supplies the same checks to creation, legacy setters and targeted edits.

`graph/editor_target.rs` resolves command ownership for the material canvas, standalone Function
Graphs, FX-Map canvases and embedded Function Graphs. Menus, keyboard shortcuts and
`ui.menu.invoke`, Library and Properties use that resolver; `ui.inspect` reports the same editor and
its canvas coordinates. Embedded editors record their owning graph and close when undo or another
command makes that owner unavailable.
Common context-menu entries live in `graph/edit_menu.rs`. Pointer gestures finish only when their
initiating button is released; Escape or focus loss cancels pending canvas movement.

Function and FX-Map rendering borrow the document, prepare screen geometry, and dispatch edits
after painting. Function wires reuse one node-layout index instead of scanning the graph for each
endpoint. The engine's `graph.function.edit` and `graph.pattern.edit` commands own creation,
movement, connections, values and result/root changes. Function targets identify standalone,
parameter, dynamic-parameter or nested FX-Map functions explicitly. Value updates may request
`merge: true`; `edit.finish` closes that undo group. The UI closes groups at gesture boundaries,
while discrete edits always receive their own undo entry.


## Files

SBS is the project format. The codec writes native filters, ports, parameters, function graphs,
pattern graphs and resources directly, without a private payload. Unsupported imports produce
warnings; unknown nodes remain placeholders. Library reference loading is not part of the current
primitive workflow. `.mcraft` is no longer supported.

The SBS `read` and `write` modules expose codec entry points. `SbsReader` and `SbsWriter` own
serialization state; focused helpers handle values, functions, patterns and canvas annotations.
Native value type codes come from `core::ValueType` rather than a second scalar type table.

See [atomic-workflow.md](atomic-workflow.md) for the tested scope and remaining limitations.

## Other apps

A `.mcmat` portable material (`formats/src/material/`, `engine/src/material_cmds.rs`) holds a
graph's cooked maps, its portable exposed inputs, presets and input nodes, and the source package.
TextureCraft paints with it and re-cooks it by running `materialcraft-cli run <package>
file.exportMaterial {path, size, values, inputs}`: the apps share the file format and the command
line, never code. Graphs keep preview images for their input nodes (`Graph::input_images`, mesh
maps from TextureCraft); `eval::cook` feeds them to the top-level graph, and a caller's images win.

Preview cooks publish the editor graph and assigned material graphs together. A synchronous cook
invalidates older worker results. Displacement uses the assigned Height source even while another
graph or an embedded function is open. Native image loading checks dimensions before decoding;
bitmap resources and graph input previews can request linear data-map reads.
