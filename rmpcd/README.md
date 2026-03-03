# rmpcd

> [!WARNING]
This project is in its very early stages and is not ready for daily use. Expect
changes to basically everything with no regards to backwards compatibility at this point.

A companion MPD client to rmpc intended to run as a daemon.

Features(so far):
* Fully configurable with Lua
* MPRIS server implementation
* on song change and on playback state change hooks

## Usage

Clone the repo and run `cargo run --release --package rmpcd` from the repo root.

Create a config file at `~/.config/rmpcd/init.lua`.

Address has the same syntax as [rmpc](https://rmpc.mierak.dev/configuration/#address)

Example:
```lua
local debounced_notify = sync.debounce(500, function(new_song)
	rmpcd.notify(new_song)
end)

rmpcd.register("on_song_change", function(old_song, new_song)
	debounced_notify(new_song)
end)

rmpcd.register("on_song_change", function(old_song, new_song)
	print("song changed from " .. old_song.file .. " to " .. new_song.file)
end)

rmpcd.register("on_state_change", function(old_state, new_state)
	print("state changed from " .. old_state .. " to " .. new_state)
end)

---@type Config
return {
	address = "@mpd",
	mpris = true,
}
```

## LuaLS type definitions

```lua
---@meta

---@alias MetadataTag string | string[]

---@class Song
---@field file string
---@field artist? MetadataTag
---@field artistsort? MetadataTag
---@field album? MetadataTag
---@field albumsort? MetadataTag
---@field albumartist? MetadataTag
---@field albumartistsort? MetadataTag
---@field title? MetadataTag
---@field titlesort? MetadataTag
---@field track? MetadataTag
---@field name? MetadataTag
---@field genre? MetadataTag
---@field mood? MetadataTag
---@field date? MetadataTag
---@field originaldate? MetadataTag
---@field composer? MetadataTag
---@field composersort? MetadataTag
---@field performer? MetadataTag
---@field conductor? MetadataTag
---@field work? MetadataTag
---@field ensemble? MetadataTag
---@field movement? MetadataTag
---@field movementnumber? MetadataTag
---@field showmovement? boolean
---@field location? MetadataTag
---@field grouping? MetadataTag
---@field comment? MetadataTag
---@field disc? MetadataTag
---@field label? MetadataTag
---@field musicbrainz_artistid? MetadataTag
---@field musicbrainz_albumid? MetadataTag
---@field musicbrainz_albumartistid? MetadataTag
---@field musicbrainz_trackid? MetadataTag
---@field musicbrainz_releasegroupid? MetadataTag
---@field musicbrainz_releasetrackid? MetadataTag
---@field musicbrainz_workid? MetadataTag

---@alias PlaybackState "Play" | "Pause" | "Stop"

---@alias HookType "on_song_change" | "on_state_change"

---@class Config
---@field address string
---@field mpris boolean
---@field password? string

---@class Logger
---@field info fun(msg: string): nil
---@field error fun(msg: string): nil
---@field debug fun(msg: string): nil
---@field warn fun(msg: string): nil
---@field trace fun(msg: string): nil

---@type Logger
log = log

---@class TimeoutHandle
---@field cancel fun(): nil

---@class Sync
---@field set_timeout fun(timeout_ms: integer, callback: fun()): TimeoutHandle
---@field debounce fun(timeout_ms: integer, callback: fun(...)): fun(...)

---@type Sync
sync = sync

---@class Process
---@field spawn fun(cmd: string[]): (integer|nil, string|nil)

---@type Process
process = process

---@class Rmpcd
---@field register fun(hook: HookType, func: fun(...): nil): nil
---@field notify fun(new_song: Song|nil): nil
---@field hooks table

---@type Rmpcd
rmpcd = rmpcd
```
