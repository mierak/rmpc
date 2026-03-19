# rmpcd

> [!WARNING]
This project is in its very early stages and is not ready for daily use. Expect
changes to basically everything with no regards to backwards compatibility at
this point.

> [!WARNING]
The README is not kept strictly up to date at the moment

A companion MPD client to rmpc intended to run as a daemon.

Features(so far):
* Fully configurable with Lua
* MPRIS server implementation
* Plugin support with custom API
* Builtin [LastFM plugin](https://github.com/mierak/rmpc/blob/master/rmpcd/src/lua/builtin/lastfm.lua)
* Builtin [Playcount tracking plugin](https://github.com/mierak/rmpc/blob/master/rmpcd/src/lua/builtin/playcount.lua)
* Builtin [Notification plugin](https://github.com/mierak/rmpc/blob/master/rmpcd/src/lua/builtin/notify.lua)
* Builtin [Lyrics download plugin](https://github.com/mierak/rmpc/blob/master/rmpcd/src/lua/builtin/lyrics.lua)

## Usage

Clone the repo and run `cargo run --release --package rmpcd` from the repo root.

Create a config file at `~/.config/rmpcd/init.lua`.

Address has the same syntax as [rmpc](https://rmpc.mierak.dev/configuration/#address)

Check `rmpcd/src/lua/builtin` for usage examples and type definitions.

Plugins run completely isolated from each other, they have separate global state etc.

Each of the builtin plugins are built in a way that they can be disabled at
runtime by sending a message to MPD channel for the corresponding plugin.
For example with rmpc's CLI:
- Disable Lastfm scrobbling: `rmpc sendmessage rmpcd.lastfm disable`
- Enable Lastfm scrobbling: `rmpc sendmessage rmpcd.lastfm enable`
- Toggle Lastfm scrobbling: `rmpc sendmessage rmpcd.lastfm toggle`

Custom plugins can be installed by `rmpcd.install`ing them as you would
`require` normal lua modules. If you have `plugin.lua` next to your `init.lua`
for example you can install it ith `rmpcd.install("plugin")` or
`plugins/custom.lua` can be installed by `rmpcd.install("plugins.custom")`.

Example:
```lua
--@type Config
local config = {}

config.address = "@mpd"
config.mpris = false

-- If you wish to subscribe to additional MPD channels
config.subscribe_channels = { "test" }

-- Automatically increment play count on song change
rmpcd.install("#builtin.playcount")

-- Install last fm scrobbling builtin
-- For now you have to request an API key yourself due to LastFM's insane API
-- design https://www.last.fm/api/account/create
rmpcd.install("#builtin.lastfm"):setup({
	api_key = "<your api key here>",
	shared_secret = "<your secret here>",
	update_now_playing = false,
})

-- Install notification on song change builtin
rmpcd.install("#builtin.notify")

-- Install the auto lyrics download builtin
rmpcd.install("#builtin.lyrics")

return config
```

## LuaLS type definitions

Type definitions are in `rmpcd/src/lua/builtin/types/`. Include them in your
config by creating a `.luarc.json` in your config root (next to init.lua). In
the future there will be a more convenient way to eject the type definitions
from rmpcd directly.

```json
{
    "workspace.library": [
        "<path to repo>/rmpcd/src/lua/builtin/types"
    ]
}
```
