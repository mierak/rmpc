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
* on song change and on playback state change hooks

## Usage

Clone the repo and run `cargo run --release --package rmpcd` from the repo root.

Create a config file at `~/.config/rmpcd/init.lua`.

Address has the same syntax as [rmpc](https://rmpc.mierak.dev/configuration/#address)

Check `rmpcd/src/lua/builtin` for usage examples and type definitions.

Example:
```lua
local notify = require("rmpcd.notify")
local playcount = require("rmpcd.playcount")
local lastfm = require("rmpcd.lastfm")
local lyrics = require("rmpcd.lyrics")

--@type Config
local config = {}

config.address = "@mpd"
config.mpris = false
config.subscribe_channels = { "test" }

-- Install the auto lyrics download builtin
lyrics.install()

-- Install notification on song change builtin
notify.install()

-- Install last fm scrobbling builtin
-- For now you have to request an API key yourself due to LastFM's insane API
-- design https://www.last.fm/api/account/create
lastfm.install({
	api_key = "<your api key>",
	shared_secret = "<your shared secret>",
})

-- Automatically increment play count on song change
playcount.install()

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
