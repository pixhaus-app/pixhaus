-- palette-harmony: append complementary or triadic swatches to the palette.
--
-- Demonstrates the Pixhaus Lua API:
--   - Reading app.fgColor
--   - HSV color access (color.hsvHue, .hsvSaturation, .hsvValue)
--   - Writing palette colors via setColor()
--   - Registering multiple command palette entries

local RAMP_STEPS = 5  -- number of lightness steps per harmony color

--- Converts HSV to RGB. Returns r, g, b in [0, 255].
local function hsv_to_rgb(h, s, v)
    h = h % 360
    local c = v * s
    local x = c * (1 - math.abs((h / 60) % 2 - 1))
    local m = v - c

    local r1, g1, b1
    if h < 60 then
        r1, g1, b1 = c, x, 0
    elseif h < 120 then
        r1, g1, b1 = x, c, 0
    elseif h < 180 then
        r1, g1, b1 = 0, c, x
    elseif h < 240 then
        r1, g1, b1 = 0, x, c
    elseif h < 300 then
        r1, g1, b1 = x, 0, c
    else
        r1, g1, b1 = c, 0, x
    end

    return
        math.floor((r1 + m) * 255 + 0.5),
        math.floor((g1 + m) * 255 + 0.5),
        math.floor((b1 + m) * 255 + 0.5)
end

--- Generates `RAMP_STEPS` colors from dark to light at the given hue.
local function make_ramp(hue, saturation)
    local ramp = {}
    for i = 1, RAMP_STEPS do
        local v = i / RAMP_STEPS
        local r, g, b = hsv_to_rgb(hue, saturation, v)
        ramp[i] = Color(r, g, b)
    end
    return ramp
end

--- Appends `colors` to `palette` starting at `start_idx`.
local function append_colors(palette, start_idx, colors)
    for i, c in ipairs(colors) do
        palette:setColor(start_idx + i - 1, c)
    end
end

--- Appends a complement ramp (hue + 180°) to the active palette.
local function add_complement()
    local sprite = app.activeSprite
    if not sprite then return end
    local palette = sprite.palette
    if not palette then return end

    local fg = app.fgColor
    local hue = fg.hsvHue
    local sat = fg.hsvSaturation
    local complement_hue = (hue + 180) % 360
    local ramp = make_ramp(complement_hue, sat)
    append_colors(palette, #palette, ramp)
end

--- Appends two triadic ramps (hue + 120° and hue + 240°).
local function add_triad()
    local sprite = app.activeSprite
    if not sprite then return end
    local palette = sprite.palette
    if not palette then return end

    local fg = app.fgColor
    local hue = fg.hsvHue
    local sat = fg.hsvSaturation
    local size = #palette
    append_colors(palette, size,         make_ramp((hue + 120) % 360, sat))
    append_colors(palette, size + RAMP_STEPS, make_ramp((hue + 240) % 360, sat))
end

app.commands.register{
    id = "palette-harmony:complement",
    title = "Add Complement Ramp to Palette",
    callbackFn = "add_complement"
}

app.commands.register{
    id = "palette-harmony:triad",
    title = "Add Triad Ramps to Palette",
    callbackFn = "add_triad"
}

_G["add_complement"] = add_complement
_G["add_triad"]      = add_triad
