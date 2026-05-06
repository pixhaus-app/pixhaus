-- color-reducer: reduce the active sprite's palette to N unique colors.
--
-- Demonstrates the Pixhaus Lua API:
--   - Reading the active sprite and its palette
--   - Iterating palette entries
--   - Writing palette colors via setColor()
--   - Registering a command palette entry
--
-- Porting from Aseprite: this script is intentionally close to the
-- Aseprite "Color Reduction" script. The primary difference is that
-- app.commands.register() replaces Aseprite's dialog-based workflow.

local MAX_COLORS = 16  -- default target palette size

--- Returns the squared Euclidean distance between two RGBA colors.
--- We compare only RGB; alpha is treated as opaque for palette reduction.
local function color_dist_sq(a, b)
    local dr = a.r - b.r
    local dg = a.g - b.g
    local db = a.b - b.b
    return dr * dr + dg * dg + db * db
end

--- Merges similar palette entries until at most `target` entries remain.
--- Uses a greedy nearest-neighbor merge: find the closest pair, merge
--- them by averaging, repeat until the palette is small enough.
---
--- @param palette  The Palette object to reduce.
--- @param target   Maximum number of colors to keep (1..256).
local function reduce_palette(palette, target)
    local size = #palette
    if size <= target then
        return
    end

    -- Collect current colors into a mutable list.
    local colors = {}
    for i = 0, size - 1 do
        local c = palette:getColor(i)
        colors[i + 1] = { r = c.r, g = c.g, b = c.b, a = c.a, active = true }
    end

    local active_count = size

    while active_count > target do
        -- Find the closest pair of still-active entries.
        local best_i, best_j, best_dist = 1, 2, math.huge
        for i = 1, #colors do
            if colors[i].active then
                for j = i + 1, #colors do
                    if colors[j].active then
                        local dist = color_dist_sq(colors[i], colors[j])
                        if dist < best_dist then
                            best_dist = dist
                            best_i = i
                            best_j = j
                        end
                    end
                end
            end
        end

        -- Merge j into i by simple average.
        local ci, cj = colors[best_i], colors[best_j]
        ci.r = math.floor((ci.r + cj.r) / 2 + 0.5)
        ci.g = math.floor((ci.g + cj.g) / 2 + 0.5)
        ci.b = math.floor((ci.b + cj.b) / 2 + 0.5)
        ci.a = math.floor((ci.a + cj.a) / 2 + 0.5)

        -- Mark the second slot as merged (will be replaced below).
        -- Replace merged slot with a copy of the surviving color so it
        -- no longer affects future distance comparisons.
        cj.active = false
        active_count = active_count - 1
    end

    -- Compact: write active entries back to the palette, zeroing the rest.
    local write_idx = 0
    for _, c in ipairs(colors) do
        if c.active then
            palette:setColor(write_idx, Color(c.r, c.g, c.b, c.a))
            write_idx = write_idx + 1
        end
    end
    -- Blank remaining slots with transparent black.
    for i = write_idx, size - 1 do
        palette:setColor(i, Color(0, 0, 0, 0))
    end
end

--- Entry point called by the command palette.
local function run()
    local sprite = app.activeSprite
    if not sprite then
        return
    end

    local palette = sprite.palette
    if not palette then
        return
    end

    reduce_palette(palette, MAX_COLORS)
end

-- Register the command in the command palette.
app.commands.register{
    id = "color-reducer:reduce",
    title = "Reduce Palette to " .. MAX_COLORS .. " Colors",
    callbackFn = "run"
}

-- Expose `run` as a global so the runtime can invoke it by name.
-- This is the callbackFn referenced above.
_G["run"] = run
