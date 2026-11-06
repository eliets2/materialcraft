# Parity with the reference workflow

> The assessment below predates the 2026-11-05 primitive rewrite. Its old library counts and
> compatibility claims are historical. See [atomic-workflow.md](atomic-workflow.md) and
> [ROADMAP.md](../ROADMAP.md) for the current scope and remaining validation.

Current integration also provides workspace-scoped `material_export`, real Height displacement
from the assigned material, explicit normal-map convention, and linear bitmap/input previews.
The native SBS and primitive-library limitations in [atomic-workflow.md](atomic-workflow.md) remain.

> **Last reviewed:** 2026-11-02 · **Last updated:** 2026-11-02 · **Change:** minor (Files 60% → 70%: portable materials for TextureCraft) · **Target:** the professional node-based material workflow (desktop app; the web app is assessed later)

**Overall: ≈ 67%** of a material artist's daily workflow works end to end (engine + UI +
automation + tests), weighted by how much of the day each area carries. **Remaining: ≈ 83 Opus
5.5 agent hours** (see [roadmap.md](roadmap.md)).

| Area | Weight | Done | What works | Main gaps |
|---|---:|---:|---|---|
| Graph editing | 15 | 75% | Pan/zoom canvas with grid; node cards with headers by category, live thumbnails and size/format labels; typed ports; bezier wires; drag to wire, re-route and unplug; select wires (click, Shift/Ctrl+click), delete them (Delete, Backspace, wire menu) and swap their sources (X); reroute dots pulled out of a wire (drag), dropped on it (double-click) or inserted from its menu; drop a wire on empty canvas to add a wired node; box/Ctrl/Shift selection; move; frames (move with contents, resize), comments (inline edit), navigation pins; node search (Space, double-click); library drag and drop; align and distribute; copy/cut/paste/duplicate (with or without links); delete and relink; disable; select upstream/downstream; create graph from selection; snapshot undo with merged slider drags | Dot-node chains editing, wire tension preferences, compact node display mode, multi-graph tabs, minimap |
| Atomic nodes | 12 | 95% | Bitmap, Blend (16 modes, masks, alpha), Blur, Channels Shuffle, Curve, Directional Blur, Directional Warp, Distance (with color propagation), Emboss, FX-Map (grid, quadtree and scatter modes with per-instance randomness), Gradient (Dynamic), Gradient Map, Grayscale Conversion, HSL, Levels, Normal, Pixel Processor, Value Processor, Sharpen, SVG (path data, fill rules, stroke), Text (own stroke font), Transformation 2D, Uniform Color, Warp, inputs, outputs, dots, graph instances | FX-Map driven by per-iteration functions, SVG arcs and SVG files, font choice for Text |
| Node library | 20 | 45% | 227 nodes: noises (tileable 3D Perlin, simplex and Worley, cracks, stones, dirt, grunge maps and more), patterns (tile generator and sampler, bricks, hexagons, weave, shape splatter, paint strokes, scratches), filters (adjustments, blending, blurs, channels, effects, normal map tools, tiling, transforms), the flood fill family, whole-material and height blending, weathering (rust, moss, leaks, edge damage, edge wear, dust) | The rest of the reference library's ~400 graphs: more grunge and scan processing, mesh-based generators and bakers, spline tools, SDF functions |
| Function graphs | 10 | 70% | A compiled, 4-wide evaluator with ~55 ops (constants, $pos/$size, exposed variables, samplers, vectors, swizzles, math, logic, comparison, noise, random, if/else); pixel and value processors; a visual function editor; dynamic parameters (a function on any numeric, color, bool or enum parameter, evaluated from the graph's inputs) | Loops, function graphs as reusable package items |
| Graph parameters and instances | 8 | 70% | Exposed inputs (float, int, bool, enum, color) bound to parameters of any compatible type, shown in groups, edited with one shared editor on the graph and on its instances; presets (save, apply, delete); sub-graph instances with their own inputs and outputs; graph attributes; `--set` on the CLI | Visibility conditions, per-instance output sizes, embedded vs. linked packages |
| 2D view | 5 | 80% | Pan/zoom, fit and 1:1, RGBA/R/G/B/A isolation, 3×3 tiling check, histogram, pixel readout, size/format line, save image | Color picker to parameters, UV overlay |
| 3D view | 10 | 55% | Rounded cube, cube, sphere, cylinder, plane, torus and material ball; OBJ, glTF and GLB meshes; base color, normal, roughness, metallic, height (parallax occlusion), AO, emissive and opacity; alpha transparency through layered surfaces (opacity output or base color alpha) and an alpha channel view; 4 procedural environments with rotation and exposure; orbit/pan/zoom; tiling; supersampling at rest | GPU renderer with tessellation, lights, material/shader switching, path-traced renderer |
| Files | 8 | 70% | `.mcraft` packages with embedded bitmaps; `.mcmat` portable materials (cooked maps, exposed inputs, presets, input nodes, the source package) that TextureCraft paints with and re-cooks through the CLI at its own resolution, values and mesh maps; preview images for input nodes; export of every output (PNG, JPEG, TGA, TIFF, BMP, WEBP, EXR, HDR; 8/16/32 bits; name pattern; sRGB vs. linear by usage); bitmap import (PNG, JPEG, TGA, TIFF, BMP, WEBP, EXR, HDR); mesh import (OBJ, glTF, GLB); open, save, import and export in the browser; atomic writes; autosave; recent files | An MCP tool for portable materials, package dependencies, SVG and font files as resources |
| UI and UX | 8 | 70% | Dockable/floatable panels (explorer, library, graph, 3D, 2D, properties, console, history), menus, main toolbar, graph toolbar with atomic quick-insert, library with live previews, properties with curve/gradient/color editors and a Levels editor over the input histogram, New graph dialog with rendered templates, export dialog, preferences (brightness, accent, scale, preview size, autosave, 3D pan limit and speed), shortcuts list, home screen at startup (samples and recent files), visual function editor, presets bar, accessible names on every widget, status bar | Translations, keymap editor, minimap, multi-graph tabs |
| Platforms | 4 | 50% | Windows, macOS, Linux, FreeBSD and web builds compile; browser file I/O; packaging scripts and release workflows | First release run |
| **Total** | **100** | **≈ 67%** | | |

Beyond the reference app: every edit is a command available through MCP, the CLI and the app's
control channel (including synthetic mouse and keyboard input), the engine runs headless with the
same results, and everything compiles to WebAssembly.

## Method

Scores are self-assessed from our code against public documentation of the professional
node-based material workflow and observation of the reference desktop app's menus, panels and
node library, area by area. A feature counts only when it works end to end (engine, UI, automation)
and is tested.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-02 | minor | ≈ 66% → ≈ 67%: `.mcmat` portable materials and TextureCraft interop (Files 70%) |
| 2026-11-02 | major | Function editor, dynamic parameters, presets, 227 nodes (FX-Map, SVG, Text, 3D noises, weathering), mesh import, browser file I/O, histogram: ≈ 51% → ≈ 66% |
| 2026-11-02 | major | First assessment after the first day of work |
