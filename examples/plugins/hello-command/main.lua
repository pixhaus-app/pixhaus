-- Hello Command — minimal Pixhaus plugin.
--
-- Registers one command in the command palette.
-- Press Ctrl+K (Cmd+K on macOS) and type "Hello" to find it.

app.commands.register {
  name    = "hello-command:greet",
  label   = "Hello: Greet",
  execute = function()
    app.alert("Hello from Hello Command!")
  end,
}
