# rayon 1.12.0 — parallel slices and strings

`ParallelSlice` / `ParallelSliceMut` add chunking, windowing, and parallel sort directly
to `[T]` (and so to `Vec<T>`, `Box<[T]>`, arrays). `ParallelString` does the same for
`str`. All three are in `use rayon::prelude::*;`. These are the workhorse APIs for pixel
buffers — a `Vec<u8>` of RGBA is a slice, and `par_chunks_mut` hands each worker a band of
scanlines.

`par_iter()` / `par_iter_mut()` on a slice are **not** methods of these traits — they come
from the `IntoParallelIterator` impls (also in the prelude) and yield `&T` / `&mut T`.

## `ParallelSlice<T: Sync>` (shared, read-only)

```rust
fn as_parallel_slice(&self) -> &[T];                                            // required

fn par_split<P>(&self, sep: P) -> Split<'_, T, P>                where P: Fn(&T) -> bool + Sync + Send
fn par_split_inclusive<P>(&self, sep: P) -> SplitInclusive<'_, T, P>  where P: Fn(&T) -> bool + Sync + Send
fn par_windows(&self, window_size: usize) -> Windows<'_, T>     // overlapping windows of len `window_size`
fn par_array_windows<const N: usize>(&self) -> ArrayWindows<'_, T, N>           // windows as &[T; N]
fn par_chunks(&self, chunk_size: usize) -> Chunks<'_, T>        // non-overlapping; last may be short
fn par_chunks_exact(&self, chunk_size: usize) -> ChunksExact<'_, T>             // drops a short remainder
fn par_rchunks(&self, chunk_size: usize) -> RChunks<'_, T>      // chunked from the end
fn par_rchunks_exact(&self, chunk_size: usize) -> RChunksExact<'_, T>
fn par_chunk_by<F>(&self, pred: F) -> ChunkBy<'_, T, F>         where F: Fn(&T, &T) -> bool + Send + Sync
```

`par_chunks(width * 4)` is the canonical pixel-band split: each item is one RGBA scanline.
`par_chunks_exact` is marginally faster when the length divides evenly (no short-tail
handling). `par_windows` is for neighborhood reads (a 3-wide kernel sees `&[a, b, c]`).

## `ParallelSliceMut<T: Send>` (mutable)

```rust
fn as_parallel_slice_mut(&mut self) -> &mut [T];                                // required

fn par_split_mut<P>(&mut self, sep: P) -> SplitMut<'_, T, P>            where P: Fn(&T) -> bool + Sync + Send
fn par_split_inclusive_mut<P>(&mut self, sep: P) -> SplitInclusiveMut<'_, T, P>
fn par_chunks_mut(&mut self, chunk_size: usize) -> ChunksMut<'_, T>             // disjoint mutable bands
fn par_chunks_exact_mut(&mut self, chunk_size: usize) -> ChunksExactMut<'_, T>
fn par_rchunks_mut(&mut self, chunk_size: usize) -> RChunksMut<'_, T>
fn par_rchunks_exact_mut(&mut self, chunk_size: usize) -> RChunksExactMut<'_, T>
fn par_chunk_by_mut<F>(&mut self, pred: F) -> ChunkByMut<'_, T, F>      where F: Fn(&T, &T) -> bool + Send + Sync

fn par_sort(&mut self)                                          where T: Ord
fn par_sort_by<F>(&mut self, compare: F)                        where F: Fn(&T, &T) -> Ordering + Sync
fn par_sort_by_key<K, F>(&mut self, f: F)                       where K: Ord, F: Fn(&T) -> K + Sync
fn par_sort_by_cached_key<K, F>(&mut self, f: F)                where F: Fn(&T) -> K + Sync, K: Ord + Send
fn par_sort_unstable(&mut self)                                 where T: Ord
fn par_sort_unstable_by<F>(&mut self, compare: F)               where F: Fn(&T, &T) -> Ordering + Sync
fn par_sort_unstable_by_key<K, F>(&mut self, f: F)              where K: Ord, F: Fn(&T) -> K + Sync
```

`par_chunks_mut` is the one to reach for: it splits the buffer into **disjoint** mutable
bands so workers never alias, which is how you mutate one `Vec<u8>` from many threads
safely. The borrow checker enforces disjointness for you.

### Which sort?

| Method | Stable? | Allocates? | Use when |
|---|---|---|---|
| `par_sort`, `par_sort_by`, `par_sort_by_key` | yes | yes — scratch the size of the slice | equal elements must keep order |
| `par_sort_by_cached_key` | yes | yes — `Vec<(K, usize)>` of keys | the key fn is expensive (compute each key once) |
| `par_sort_unstable*` | no | **no** — in place | the default; faster, no allocation |

Rule of thumb: reach for `par_sort_unstable*` unless you specifically need stability. For
sorting a palette by hue/luminance, the unstable variants are the right call.

## `ParallelString` (on `str`)

```rust
fn as_parallel_string(&self) -> &str;                                           // required

fn par_chars(&self) -> Chars<'_>                       // -> char
fn par_char_indices(&self) -> CharIndices<'_>          // -> (usize, char)  byte index + char
fn par_bytes(&self) -> Bytes<'_>                       // -> u8  (multi-byte UTF-8 never split across threads)
fn par_encode_utf16(&self) -> EncodeUtf16<'_>          // -> u16
fn par_split<P: Pattern>(&self, sep: P) -> Split<'_, P>            // -> &str
fn par_split_inclusive<P: Pattern>(&self, sep: P) -> SplitInclusive<'_, P>
fn par_split_terminator<P: Pattern>(&self, term: P) -> SplitTerminator<'_, P>   // trailing empty omitted
fn par_lines(&self) -> Lines<'_>                       // -> &str  line endings stripped
fn par_split_whitespace(&self) -> SplitWhitespace<'_>
fn par_split_ascii_whitespace(&self) -> SplitAsciiWhitespace<'_>
fn par_matches<P: Pattern>(&self, pat: P) -> Matches<'_, P>        // -> &str  matched substrings
fn par_match_indices<P: Pattern>(&self, pat: P) -> MatchIndices<'_, P>  // -> (usize, &str)
```

`P: Pattern` accepts a `char`, a `&[char]` set, or a `Fn(char) -> bool + Sync + Send`
closure. The `Pattern` trait is sealed (`#[doc(hidden)]`) — you can't implement it, only
pass the supported forms; this matches the "sealed traits where extension is internal-only"
convention in [[pixhaus-rust-conventions]]. Strings are a minor use in a pixel editor —
relevant for parsing a palette file or a large `.gpl`/CSV import where lines are
independent.

`rayon::string::Drain` is the parallel draining iterator for an owned `String` (removes a
char range while keeping capacity), obtained via the `String` `par_drain` API rather than
this trait.

## Pixhaus patterns

```rust
use rayon::prelude::*;

// Per-scanline fill over the dirty rows of an RGBA8 buffer (width px, 4 bytes/px).
// Slice to the dirty band FIRST so work scales with the edit, not the canvas.
let stride = width * 4;
let band = &mut buffer[first_row * stride..(last_row + 1) * stride];
band.par_chunks_mut(stride)
    .for_each(|row| for px in row.chunks_exact_mut(4) { px.copy_from_slice(&fill_rgba) });

// Premultiply alpha across the whole buffer, one pixel per item.
buffer.par_chunks_exact_mut(4).for_each(|px| {
    let a = px[3] as u16;
    for c in &mut px[..3] { *c = ((*c as u16 * a) / 255) as u8; }
});

// Sort a palette by luminance (unstable: no allocation, order of ties doesn't matter).
palette.par_sort_unstable_by_key(|&[r, g, b, _]| {
    (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000
});
```

For a tiny dirty region the sequential `chunks_mut` may win — `par_chunks_mut` pays a split
cost. Add `.with_min_len(n)` on the resulting iterator (it's indexed) to stop rayon
splitting a small band across threads, or branch to a sequential path below a size
threshold. Bound the work by the dirty rectangle; that's the [[8k-perf-constraint]] in one
sentence.
