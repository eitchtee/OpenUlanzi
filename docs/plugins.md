# Writing a plugin

A plugin adds to OpenUlanzi with an ordinary program, in any language, that reads one JSON object per line from stdin and writes them to stdout. It can add three things:

- **Actions**, which a user puts on a key alongside any other action.
- **Buttons**, where the plugin decides everything a key shows and does.
- **Pages**, where the plugin decides everything the 13 keys and the wide screen show and do.

[`examples/plugins/notes`](../examples/plugins/notes) is a complete plugin with an action, [`examples/plugins/tally`](../examples/plugins/tally) one with a page and a button, and [`examples/plugins/codex-minibar`](../examples/plugins/codex-minibar) a real one that shows AI quotas from Codex Minibar.

Plugins are not sandboxed: they run with the user's full permissions. Only install ones you trust.

## Installing

Put the plugin's folder in `%APPDATA%\OpenUlanzi\plugins` (`~/Library/Application Support/OpenUlanzi/plugins` on macOS), then quit from the tray and reopen OpenUlanzi. Settings lists each plugin and any problem starting it. With the headless runner, the `plugins` folder sits next to the config file.

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

Pages and buttons are declared the same way as actions, and their ids share one list:

```toml
[[page]]
id = "scores"
name = "Scoreboard"
icon = "ph:trophy-bold"
fields = [{ key = "league", label = "League", default = "NBA" }]

[[button]]
id = "light"
name = "Light"
fields = [{ key = "light", label = "Light", options = "lights" }]
```

Users add a page with the puzzle button next to the page tabs, and drag a button onto a control from the action list.

A program in the plugin's folder wins over one on the `PATH`, so `["node", "main.js"]` also works if Node is installed.

Every field is text. `hint`, `default` and `options` are optional. `options` names a list the plugin sends, which the editor offers as a dropdown. Until it has one, as when the plugin isn't running, the user types the value instead.

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

## Pages and buttons

Each placement with the same id and fields is one **instance**, wherever it sits and on however many decks, so it shows the same thing everywhere. To tell two apart, give them different fields.

OpenUlanzi says when an instance comes onto a deck, whether by going to its page or by holding a key that shows it:

```json
{"type":"appear","instance":"com.example.hue/4f2a91c07d3e1b66","view":"light","params":{"light":"Desk"}}
```

and when it has left every deck:

```json
{"type":"disappear","instance":"com.example.hue/4f2a91c07d3e1b66"}
```

Treat `instance` as an opaque name. It is not stable between OpenUlanzi versions, so don't save anything under it. Draw on `appear`, and stop updating on `disappear`, because nobody can see it.

### Drawing

A button gets one `button`:

```json
{"type":"draw","instance":"...","button":{"label":"Desk","icon":"ph:lightbulb-fill","color":"#fbbf24","action":{"plugin":{"id":"com.example.hue","action":"toggle","params":{"light":"Desk"}}}}}
```

A page gets `keys`, named `0` to `12` and `wide`. Keys left out stay as they are and `null` empties one, so a clock only has to send the key that changed:

```json
{"type":"draw","instance":"...","keys":{"3":{"label":"Home"},"wide":{"svg":"<svg ...>"},"7":null}}
```

Each of those is a whole button, exactly as written in the config, so it holds what the key looks like and what it does:

- **Look:** `label`, `icon`, `color`, `background`, `image`, `svg`. Colors are `#rrggbb`. Paths must be absolute.
- **Doing:** `action`, `double`, `hold`, `hold_ms`, and on a knob `left`, `right`, `press_left` and `press_right`.

Any action works, not only the plugin's own: `"back"`, `{"page":"Other"}`, `{"hotkey":"ctrl+c"}`. To hear about a press, use a `plugin` action as above. There is no separate press event, so hold times and double clicks work the same as anywhere else.

`svg` is a whole SVG document, for drawing anything at all. A key is 196 by 196 and the wide screen 458 by 196; other sizes are scaled to fit. It sits over `background` and under `label`. System fonts work. A key that also gets `display` shows a firmware face on the wide screen instead, as in the config.

If any part of a `draw` is wrong, none of it applies. The deck takes about three updates a second; faster ones are skipped and the latest wins.

To forget everything an instance drew, send:

```json
{"type":"clear","instance":"..."}
```

### How they behave

- A button looks as the user set it in the editor until the plugin draws it, and again once the plugin stops. While the plugin isn't drawing it, it does nothing.
- A page's knobs and side buttons stay the user's. If the user keeps a Back key on the page, the first key is not the plugin's: anything drawn there is hidden.
- **Give a page a way out.** A page takes over all the keys, so put a `"back"` action on one of them. On a plugin page it returns to where the user came from, or to the previous page if they arrived by `Next page`. While nothing is drawn, as when the plugin isn't running, OpenUlanzi puts its own Back on the first key.

## Lifecycle

- **Exit when stdin closes.** That is how a plugin learns OpenUlanzi has gone.
- A plugin that exits is started again after 3 seconds. Keys pressed in between are dropped, not queued, and what it drew is forgotten. The new process gets an `appear` for every instance still on a deck.
- Handle slow work so that the next line still gets read, or actions will pile up behind it.
