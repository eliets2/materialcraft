# File format parity

> The assessment below predates the 2026-11-05 primitive rewrite. Its old library counts and
> compatibility claims are historical. See [atomic-workflow.md](atomic-workflow.md) and
> [ROADMAP.md](../ROADMAP.md) for the current scope and remaining validation.

Current integration also provides workspace-scoped `material_export`, real Height displacement
from the assigned material, explicit normal-map convention, and linear bitmap/input previews.
The native SBS and primitive-library limitations in [atomic-workflow.md](atomic-workflow.md) remain.

> **Last reviewed:** 2026-11-04 · **Last updated:** 2026-11-04 · **Change:** major (SBS is the main project format) · **Target:** the professional node-based material workflow

SBS packages are MaterialCraft's main project format (issue #49), read and written with its own
clean-room code from the format's public documentation and sample files. The older `.mcraft`
package is still read and written. MaterialCraft also reads and writes the open image formats
material artists exchange.

| Format | Read | Write | Fidelity | Tests |
|---|---|---|---|---|
| `.sbs` package (XML), the main format | Yes: atomic filters, library nodes with a MaterialCraft node of the same name (88 of the 491 in `assets/sbs/all_nodes.sbs`), graph instances, graph inputs/outputs, exposed values, parameters linked to inputs, base parameters, frames, comments, embedded and linked bitmaps and SVGs. Anything else (missing nodes, curve points, gradient keys, function graphs, environment maps) is a Console warning; missing nodes stay as placeholders | Yes: SBS data with bitmaps embedded, plus MaterialCraft's exact copy | Lossless for files MaterialCraft saved (exact copy, guarded by a content hash); placeholders round-trip their SBS data | `formats::sbs::tests`, engine `sbs_is_the_default_save_format_and_mcraft_still_works`, `an_sbs_file_with_missing_nodes_opens_softly` |
| `.mcraft` package (zip: `package.json` + `resources/`), older | Yes | Yes | Lossless (graphs, parameters, frames, comments, pins, exposed inputs, embedded bitmaps) | `formats::tests::package_round_trip`, engine `package_save_and_open_round_trip` |
| PNG | Yes (8/16-bit, gray and color) | Yes (8/16-bit) | 16-bit gray round-trips within 1/65535 | `formats::tests::export_every_format` |
| JPEG | Yes | Yes (8-bit) | Lossy | same |
| TGA | Yes | Yes (8-bit) | Lossless | same |
| TIFF | Yes | Yes (8/16-bit) | Lossless | same |
| BMP | Yes | Yes (8-bit) | Lossless | same |
| WEBP | Yes | Yes (8-bit lossless) | Lossless | same |
| OpenEXR | Yes | Yes (32-bit float RGB) | Float | same |
| Radiance HDR | Yes | Yes | Float (RGBE) | same |
| SVG, fonts (resources) | No | — | — | gap |
| Meshes for the 3D view (OBJ, glTF, FBX) | No | — | — | gap |
| `.mcmat` portable material (zip: manifest, cooked maps, source package) | Yes (opens the source package) | Yes | Maps at 8-bit sRGB (color) / 16-bit linear (data); exposed inputs, presets and input nodes in the manifest. Our own format, shared with TextureCraft, not the reference app's archive format | `formats::material::tests`, engine `material_cmds::tests`, TextureCraft `materialcraft_link` |

Exports write base color, emissive and specular through the sRGB curve and data maps (normal,
roughness, metallic, height, ambient occlusion, opacity) linear.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-04 | major | SBS packages become the main project format; `.mcraft` kept |
| 2026-11-02 | minor | `.mcmat` portable materials replace the published-archive gap |
| 2026-11-02 | major | First version |
