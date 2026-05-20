# pixhaus-vectorize

Centerline vectorization for raster ink layers. Public API:

```rust
let vi = centerline_vectorize(&raster, &palette, &config)?;
```

The pipeline runs four stages:

1. Polygonize the ink mask into closed Moore-neighbor contours.
2. Skeletonize via two-pass 3-4 chamfer distance transform plus
   Zhang-Suen thinning. The distance transform supplies the local
   stroke half-width per skeleton pixel.
3. Organize the medial-axis graph by pruning branches shorter than
   `config.min_segment_length` and merging through degree-2 nodes.
4. Fit one stroke per remaining edge with Ramer-Douglas-Peucker
   simplification at `config.simplify_tolerance` and
   thickness-weighted vertex construction.

## Adaptations

The pipeline is adapted from OpenToonz under BSD-3-Clause. Source map:

- `toonz/sources/toonzlib/centerlinepolygonizer.cpp` -> `src/contour.rs`
- `toonz/sources/toonzlib/centerlineskeletonizer.cpp` -> `src/skeleton.rs`
- `toonz/sources/toonzlib/tcenterlinevectorizer.cpp` -> `src/lib.rs` and
  `src/organize.rs`
- `toonz/sources/toonzlib/centerlinetostroke.cpp` -> `src/stroke_fit.rs`

We use distance-transform + Zhang-Suen thinning for skeletonization
rather than OpenToonz's Voronoi-of-vertices approach; both yield
equivalent medial-axis graphs for raster ink.

See the repo-root `THIRD_PARTY_NOTICES.md` for the BSD-3-Clause grant
and Dwango copyright.
