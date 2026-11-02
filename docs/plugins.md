# Writing a plugin

A plugin adds actions to OpenUlanzi. It is an ordinary program, in any language, that reads one JSON object per line from stdin. [`examples/plugins/notes`](../examples/plugins/notes) is a complete one in a few lines of PowerShell.

Plugins are not sandboxed: they run with the user's full permissions. Only install ones you trust.

## Installing

Put the plugin's folder in `%APPDATA%\OpenUlanzi\plugins`, then quit from the tray and reopen OpenUlanzi. Settings lists each plugin and any problem starting it. With the headless runner, the `plugins` folder sits next to the config file.

## plugin.toml

```toml
protocol = 1                    # always 1 for now
id = "com.example.hue"          # what configs refer to, so keep it stable
name = "Philips Hue"            # the group in the action list
command = ["hue-plugin.exe"]    # program and arguments, run from the plugin's folder

[[action]]
id = "toggle_light"
name = "Toggle light"
icon = "ph:lightbulb-bold"      # optional, any icon from the app's icon picker
fields = [
  { key = "light", label = "Light", hint = "Shown under the box.", default = "", options = "lights" },
]
```

A program in the plugin's folder wins over one on the `PATH`, so `["node", "main.js"]` also works if Node is installed.

Every field is text. `hint`, `default` and `options` are optional. `options` names a list the plugin sends to suggest values; the user can still type anything, because the plugin may not be running while they edit.

## Messages

OpenUlanzi starts one process when it starts, shared by all decks, and sends this for each action:

```json
{"type":"run","action":"toggle_light","params":{"light":"Desk"}}
```

All `params` values are strings. Gestures are handled before this point: a click, a hold and each knob direction are separate actions in the config.

A plugin can send this on stdout at any time, to set or replace a list of suggestions:

```json
{"type":"options","key":"lights","values":["Desk","Ceiling"]}
```

Send complete lines and flush after each one. Anything else on stdout is ignored, so write logs to stderr, which ends up in `plugin.log` in the plugin's folder.

## Lifecycle

- **Exit when stdin closes.** That is how a plugin learns OpenUlanzi has gone.
- A plugin that exits is started again after 3 seconds. Keys pressed in between are dropped, not queued.
- Handle slow work so that the next line still gets read, or actions will pile up behind it.
