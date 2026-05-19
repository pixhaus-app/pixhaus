# Aseprite migration notes

Pixhaus reads and writes `.aseprite` files but its data model is a
superset of Aseprite's. Some constructs lose fidelity on export. This
document records each downgrade so the affected user can decide whether
to switch to `.pixhaus` (the native format) to preserve the original
state.

## Blend mode loss on Aseprite export

Pixhaus blend modes `LinearBurn`, `DarkerColor`, `LinearDodge`,
`LighterColor`, `VividLight`, `LinearLight`, `PinLight`, and `HardMix`
have no equivalent in Aseprite's file format. Exporting a project
containing these modes downgrades each affected layer to `Normal` and
emits a `tracing::warn!` of the form:

```
WARN aseprite: Aseprite has no equivalent for this blend mode; \
    downgrading to Normal on export layer=<name> blend_mode=<mode>
```

The downgrade is one-way: re-importing the resulting `.aseprite` file
will load those layers as `Normal`, not as the original mode. To
preserve the original blend modes, export to `.pixhaus` instead.

These eight modes are adapted from OpenToonz's
`toonz/sources/stdfx/igs_color_blend.cpp` and arrived in Pixhaus
under stream S55.
