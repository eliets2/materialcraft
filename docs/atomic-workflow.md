# Primitive editing and native SBS

MaterialCraft is a clean-room implementation. The Rust catalog contains only native primitives
and graph input/output items. SBS library nodes belong in SBS graphs; their former Rust
implementations have been removed.

## Try it

Choose **File → New → Atomic sample**, or open [`examples/atomic_sample.sbs`](../examples/atomic_sample.sbs).
The sample connects a value input to a multiplication function, feeds that value into a pixel
function, combines its gradient with an iterated pattern, and blurs the result. It has an image
output and a value output.

Use the library or Space search to add primitives and drag between ports to connect them.
Select a Value Processor or Pixel Processor and choose **Edit function** in Properties.
The function editor supports adding operations, editing constants, wiring ports and choosing
the result. Select an FX-Map and press **Ctrl+E**, or right-click it and choose **Edit graph**.
This enters its subgraph in the main canvas. Add Quadrant, Switch, Iterate or vertical Dot nodes, drag from
bottom ports to top ports, and choose **Set as root** from a node’s context menu. Alt-click a port
to disconnect it. Selected-node parameters appear in Properties. Click the parent breadcrumb
or press **Alt+Left** to return. Positions and connections are saved in SBS.

New FX-Map nodes start with a Quadrant root. Their main-graph inputs start with Background and
Input image 0; connecting an image exposes the next image input. Inputs appear progressively as they are connected; connected pins stay visible. Main and Function Graph pin names appear on node hover. Internal FX-Map pins have no visible names.

Value inputs on processors have explicit types. A connection with an incompatible type is
rejected. All edits use engine commands and support undo. Save and reopen through the usual
File commands.

The same sample is available through `file.newAtomicSample`. Regenerate the file with:

```sh
cargo +1.96.0 run -p materialcraft-engine --release --example atomic_sample -- examples/atomic_sample.sbs
```

## Catalog

| Context | Entries |
|---|---:|
| Main graph | 26 native primitives, including processors and reroute, plus 4 input/output items |
| Functions | 85 native operations |
| FX-Map graph | Quadrant (`paramset`), Switch (`markov2`), Iterate (`addnode`), Dot (`passthrough`) |

Image primitives have individual files in `crates/nodes/src/atomic/`. Function evaluation and
pattern traversal have separate modules. Document types each live in their own file under
`crates/core/src/`. SBS parsing and writing live under `crates/formats/src/sbs/`.

## Files and execution

The saved document is native SBS XML: filters, graph bridges, typed value ports, function nodes,
pattern nodes, parameters and resources. There is no private package comment or embedded JSON
document. `.mcraft` read/write support has been removed. Portable `.mcmat` materials now contain
`source.sbs` and use manifest version 2; older archives are rejected.

Function graphs and pattern graphs use iterative traversal, detect cycles and enforce node and
execution budgets. A shared child in a DAG is legal. Exhausting a budget produces an error.
SBS library reference resolution is outside this change.

## Current limits

- Reopening the generated SBS in the reference application has not been verified
- Native IDs, ports and types are mapped, but numerical parity with the reference renderer has
  not been established, including pattern shapes, blending, sampling and random sequences
- Text renders the original built-in stroke font; external font references produce an explicit
  error. SVG supports vector geometry, without external images or font loading
- Some native parameter variants and package/editor metadata remain unsupported; import reports
  unsupported parameters. This is not a lossless importer for arbitrary SBS files
Pixel Processor, Value Processor and FX-Map image inputs grow as connected, up to 256.

The tests cover primitive cooking, native field round trips, scalar/vector values, wire type
checks, undo, embedded SVG, function control flow, cycle rejection and execution budgets.

## Graph editing

Node placement and dragging snap to the 32-unit grid by default. The graph toolbar and Tools
menu toggle snapping. Function nodes use the same setting. Opening an SBS preserves its saved
positions; snapping takes effect when nodes are placed or moved.

A new Value Processor has a Float input named `value1` and a function that returns it. Its
separate `Image 0` input is for image sampling. Drag an Input Value output to the orange value
input on each processor to branch the output. Additional typed inputs can be added in Properties. Port labels display readable names such as
“Value 1” while preserving native identifiers such as `value1` for wiring and SBS files.
Reconnecting a wire is one undo step; an invalid drop leaves the original connection intact.

The graph context menu offers a flat Add node list, annotations, output views, thumbnails,
saving, clipboard actions, navigation pins and selection actions. Paste and Duplicate retain
incoming links within their source graph; their “without link” variants retain only wires
between copied nodes. F5 clears thumbnails; Compute nodes thumbnails restores them. F2 and
Shift+F2 cycle through navigation pins. Pin names, positions and order persist as native SBS
canvas objects in both Graph and Function Graph files.


Frames select by their background, title or border and show an outer selection outline. Properties
edits title visibility, title, fill color and description. Comments are plain canvas text with a
Description property. Moving a frame carries enclosed nodes, comments, nested frames and navigation
pins by one shared offset, with one undo step. Resizing changes only the frame bounds.

New graph creation offers **Graph** and **Function Graph**. Function Graphs are package items with
a flat operation menu, editable constants, typed native operations, wiring and a designated output
node. They save as native SBS `<function>` items and reopen in MaterialCraft. Calling other SBS
function graphs remains outside this change. The reference application's ability to reopen the
saved files still needs external verification.

Function Graphs share the main canvas card geometry and styling, including compact Dot nodes.
The Graph, Function Graph and FX-Map Add node menus remain flat, with dividers between groups
and room to use the window height. Type a letter to enable names starting with it while keeping the full list visible;
Backspace clears the filter.
