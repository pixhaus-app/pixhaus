// GLSL shader sources for the canvas viewport renderer.
//
// Five programs:
//   SPRITE      — sprite tiles (checkerboard bg + optional texture)
//   GRID        — procedural per-pixel grid overlay (visible at zoom >= 4)
//   MAJOR_GRID  — configurable major-grid overlay (every N canvas pixels)
//   ONION       — previous/next-frame tile tinted red/blue and additively blended
//   ANTS        — marching-ants selection outline (time-animated dashes)

// ── Shared helpers ─────────────────────────────────────────────────────────

// Vertex shader used by all three programs.
// Accepts canvas-coordinate positions and transforms them to clip space.
export const COMMON_VERT = /* glsl */ `#version 300 es
precision highp float;

in vec2 a_pos;

uniform vec2 u_resolution; // viewport element size in CSS pixels
uniform vec2 u_scroll;     // canvas coord that maps to viewport centre
uniform float u_zoom;      // CSS pixels per canvas pixel

out vec2 v_screen; // screen-space position (CSS px, origin = top-left)

void main() {
  vec2 screen = (a_pos - u_scroll) * u_zoom + u_resolution * 0.5;
  v_screen = screen;
  vec2 clip = (screen / u_resolution) * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
}
`;

// ── Sprite program ─────────────────────────────────────────────────────────

// Fragment shader for sprite tiles.
//
// When u_has_tile is false (no GPU data yet), draws a procedural checkerboard
// that represents the transparent canvas.  When u_has_tile is true, alpha-
// composites the RGBA texture over the same checkerboard background.
//
// Precision must match COMMON_VERT (highp) or WebGL2 rejects the program
// link with "Precisions of uniform 'u_zoom' differ between VERTEX and
// FRAGMENT shaders." `highp` is guaranteed available in fragment shaders
// per the WebGL2 baseline; this is a desktop target and the precision
// difference has no measurable performance impact.
export const SPRITE_FRAG = /* glsl */ `#version 300 es
precision highp float;

in vec2 v_screen;

uniform sampler2D u_tile;
uniform bool u_has_tile;
uniform vec2 u_tile_origin_canvas; // tile top-left in canvas coordinates
uniform vec2 u_tile_size_canvas;   // tile dimensions in canvas pixels
uniform float u_zoom;
uniform vec2 u_scroll;
uniform vec2 u_resolution;

out vec4 out_color;

// 8×8 canvas-pixel checker squares in two shades of grey.
vec4 checkerboard(vec2 canvas_pos) {
  vec2 cell = floor(canvas_pos / 8.0);
  float even = mod(cell.x + cell.y, 2.0);
  float v = even < 0.5 ? 0.47 : 0.58;
  return vec4(v, v, v, 1.0);
}

void main() {
  // Recover canvas coordinates from screen position.
  vec2 canvas_pos = (v_screen - u_resolution * 0.5) / u_zoom + u_scroll;

  vec4 bg = checkerboard(canvas_pos);

  if (u_has_tile) {
    vec2 uv = (canvas_pos - u_tile_origin_canvas) / u_tile_size_canvas;
    vec4 tex = texture(u_tile, uv);
    // Pre-multiplied alpha blend over checker.
    out_color = vec4(mix(bg.rgb, tex.rgb, tex.a), 1.0);
  } else {
    out_color = bg;
  }
}
`;

// ── Grid program ────────────────────────────────────────────────────────────

// Fragment shader for the pixel grid overlay.
//
// Draws a 1-CSS-px border between every canvas pixel at the fractional
// boundary. Uses fwidth-based anti-aliasing to keep lines stable during
// continuous zoom without aliasing flicker.
export const GRID_FRAG = /* glsl */ `#version 300 es
precision highp float;

in vec2 v_screen;

uniform float u_zoom;
uniform vec2 u_scroll;
uniform vec2 u_resolution;

out vec4 out_color;

void main() {
  // Canvas position of this fragment.
  vec2 canvas_pos = (v_screen - u_resolution * 0.5) / u_zoom + u_scroll;

  // Distance to the nearest canvas-pixel boundary, in canvas pixels.
  vec2 frac = fract(canvas_pos);
  vec2 dist = min(frac, 1.0 - frac); // 0 at boundaries, 0.5 at centres

  // Scale to CSS pixels for stable width.
  vec2 dist_css = dist * u_zoom;

  // Draw a 0.75-CSS-px line at each boundary.
  float line_w = 0.75;
  float edge_x = smoothstep(line_w, line_w + 1.0, dist_css.x);
  float edge_y = smoothstep(line_w, line_w + 1.0, dist_css.y);
  float edge = 1.0 - edge_x * edge_y;

  if (edge < 0.01) discard;

  // Semi-transparent dark line that works on any background.
  out_color = vec4(0.0, 0.0, 0.0, edge * 0.2);
}
`;

// ── Marching-ants program ───────────────────────────────────────────────────

// Fragment shader for the animated selection outline.
//
// Renders a 1-CSS-px dashed border around a rectangle in canvas coordinates.
// u_time advances each frame; the dash offset shifts to create the marching
// effect.  The vertex shader positions a quad that covers the border region.
export const ANTS_VERT = /* glsl */ `#version 300 es
precision highp float;

in vec2 a_pos; // canvas coordinates

uniform vec2 u_resolution;
uniform vec2 u_scroll;
uniform float u_zoom;

out vec2 v_screen;
out vec2 v_canvas; // canvas coordinate for dash math

void main() {
  vec2 screen = (a_pos - u_scroll) * u_zoom + u_resolution * 0.5;
  v_screen = screen;
  v_canvas = a_pos;
  vec2 clip = (screen / u_resolution) * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
}
`;

export const ANTS_FRAG = /* glsl */ `#version 300 es
precision highp float;

in vec2 v_screen;
in vec2 v_canvas;

// Selection rect in canvas coordinates.
uniform vec2 u_sel_min;
uniform vec2 u_sel_max;
uniform float u_time;   // seconds, used for march speed
uniform float u_zoom;

out vec4 out_color;

void main() {
  // Perimeter coordinate: accumulate distance along edges.
  vec2 sz = u_sel_max - u_sel_min;
  float perimeter = 2.0 * (sz.x + sz.y);

  // Determine how close this fragment is to an edge (in canvas pixels).
  vec2 local = v_canvas - u_sel_min;
  float d_left   = local.x;
  float d_right  = sz.x - local.x;
  float d_top    = local.y;
  float d_bottom = sz.y - local.y;
  float d_edge   = min(min(d_left, d_right), min(d_top, d_bottom));

  // Only draw within 1 canvas pixel of the border.
  float border_canvas = 1.5 / u_zoom; // 1.5 CSS px in canvas coords
  if (d_edge > border_canvas) discard;

  // Arc length along the perimeter for this fragment.
  float arc = 0.0;
  if (d_left < d_right && d_left < d_top && d_left < d_bottom) {
    arc = sz.y + sz.x + (sz.y - local.y); // left edge (bottom-up)
  } else if (d_top < d_right && d_top < d_bottom) {
    arc = local.x; // top edge (left-right)
  } else if (d_right < d_bottom) {
    arc = sz.x + local.y; // right edge (top-down)
  } else {
    arc = sz.x + sz.y + (sz.x - local.x); // bottom edge (right-left)
  }

  // Marching offset: 8-canvas-pixel period, 4 on / 4 off.
  float period = 8.0;
  float march_speed = 30.0; // canvas pixels per second
  float t = arc + u_time * march_speed;
  float dash = step(period * 0.5, mod(t, period));

  // Alternate between black and white dashes.
  vec3 color = dash > 0.5 ? vec3(1.0) : vec3(0.0);
  out_color = vec4(color, 0.9);
}
`;

// ── Major-grid program ──────────────────────────────────────────────────────

// Fragment shader for the configurable major-grid overlay.
//
// Draws a 1-CSS-px line every `u_spacing` canvas pixels in both axes.  Used to
// visualise tile boundaries when the user is editing tilemaps or sprite sheets.
// Reuses COMMON_VERT.
export const MAJOR_GRID_FRAG = /* glsl */ `#version 300 es
precision highp float;

in vec2 v_screen;

uniform float u_zoom;
uniform vec2 u_scroll;
uniform vec2 u_resolution;
uniform float u_spacing; // canvas pixels between major-grid lines

out vec4 out_color;

void main() {
  vec2 canvas_pos = (v_screen - u_resolution * 0.5) / u_zoom + u_scroll;

  // Distance to nearest grid line, in canvas pixels.
  vec2 q = mod(canvas_pos, u_spacing);
  vec2 dist = min(q, u_spacing - q);

  vec2 dist_css = dist * u_zoom;
  float line_w = 1.0;
  float edge_x = smoothstep(line_w, line_w + 1.0, dist_css.x);
  float edge_y = smoothstep(line_w, line_w + 1.0, dist_css.y);
  float edge = 1.0 - edge_x * edge_y;

  if (edge < 0.01) discard;

  // Cyan-tinted line, slightly brighter than the per-pixel grid so the two
  // overlays read distinctly when both are on.
  out_color = vec4(0.4, 0.7, 1.0, edge * 0.45);
}
`;

// ── Onion-skin program ──────────────────────────────────────────────────────

// Fragment shader for onion-skin overlay tiles.
//
// Samples one tile texture, multiplies its rgb by u_tint, scales alpha by
// u_opacity, and outputs the result.  The renderer alpha-blends multiple
// passes — one per neighbour frame — additively over the current frame.
// Reuses COMMON_VERT.
export const ONION_FRAG = /* glsl */ `#version 300 es
precision highp float;

in vec2 v_screen;

uniform sampler2D u_tile;
uniform vec2 u_tile_origin_canvas;
uniform vec2 u_tile_size_canvas;
uniform vec3 u_tint;        // rgb multiplier (e.g. red for prev, blue for next)
uniform float u_opacity;    // 0..1 final alpha multiplier
uniform float u_zoom;
uniform vec2 u_scroll;
uniform vec2 u_resolution;

out vec4 out_color;

void main() {
  vec2 canvas_pos = (v_screen - u_resolution * 0.5) / u_zoom + u_scroll;
  vec2 uv = (canvas_pos - u_tile_origin_canvas) / u_tile_size_canvas;
  vec4 tex = texture(u_tile, uv);
  // Tint the rgb channel; preserve alpha shape but scale by opacity.
  out_color = vec4(tex.rgb * u_tint, tex.a * u_opacity);
}
`;
