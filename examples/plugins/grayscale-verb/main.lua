-- Grayscale Verb — desaturate the active layer.
--
-- Demonstrates the AI verb lifecycle: run() receives a VerbContext,
-- produces effects, and the editor shows a preview before committing.
-- This verb uses no inference backend — the desaturation is classical
-- pixel math so it runs instantly and costs nothing.

app.ai.registerVerb {
  id          = "com.pixhaus.examples.grayscale",
  label       = "Grayscale",
  description = "Desaturate the active layer using Rec. 601 luminance.",
  cost        = { credits = 0 },

  run = function(ctx)
    local layer = ctx.activeLayer
    if not layer then
      return { error = "No active layer." }
    end

    local effects = {}

    for _, frame in ipairs(ctx.sprite.frames) do
      local cel = layer:cel(frame)
      if cel then
        local src = cel.image
        local dst = src:clone()

        for px in dst:pixels() do
          local c = px()
          if c.alpha > 0 then
            -- Rec. 601 luma: matches human perception of brightness.
            local lum = math.floor(c.red * 0.299 + c.green * 0.587 + c.blue * 0.114)
            px(Color { r = lum, g = lum, b = lum, a = c.alpha })
          end
        end

        table.insert(effects, {
          kind   = "replace_cel",
          layer  = layer,
          frame  = frame,
          pixels = dst,
        })
      end
    end

    if #effects == 0 then
      return { error = "No cels found on the active layer." }
    end

    return {
      summary = string.format("Desaturate layer \"%s\" (%d cel%s)",
        layer.name, #effects, #effects == 1 and "" or "s"),
      effects = effects,
    }
  end,
}
