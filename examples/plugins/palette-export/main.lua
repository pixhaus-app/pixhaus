-- Palette Export — exports the active sprite's palette as a CSV file.
--
-- Written to ~/Documents/palette.csv.
-- Requires the fs permission for ~/Documents (declared in plugin.toml).
-- Pixhaus will prompt the user to approve the path on first use.

app.commands.register {
  name    = "palette-export:export-csv",
  label   = "Palette: Export as CSV",

  enabled = function()
    return app.activeSprite ~= nil
  end,

  execute = function()
    local sprite = app.activeSprite
    if not sprite then
      app.alert("No sprite is open.")
      return
    end

    local pal   = sprite.palette
    local lines = { "index,hex,name" }

    for i = 0, #pal - 1 do
      local c   = pal:getColor(i)
      local hex = string.format("#%02x%02x%02x", c.red, c.green, c.blue)
      local name = c.name or ""
      -- Escape commas inside color names.
      if name:find(",") then name = '"' .. name .. '"' end
      table.insert(lines, string.format("%d,%s,%s", i, hex, name))
    end

    local path = app.fs.joinPath(app.fs.userDocsPath, "palette.csv")
    local file = io.open(path, "w")
    if not file then
      app.alert("Could not open " .. path .. " for writing.\n\nCheck that the path exists and is writable.")
      return
    end
    file:write(table.concat(lines, "\n"))
    file:close()

    app.alert(string.format("Exported %d colors to:\n%s", #pal, path))
  end,
}
