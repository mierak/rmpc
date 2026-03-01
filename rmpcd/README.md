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

Create a config file at `~/.config/rmpcd/init.lua`.

Address has the same syntax as [rmpc](https://rmpc.mierak.dev/configuration/#address)

Example:
```lua
local song_changes = 0
local state_changes = 0

---@type Config
return {
	address = "127.0.0.1:6600",
	mpris = true,
	on_song_change = function(old_song, new_song)
		song_changes = song_changes + 1
		if new_song == nil then
			log.info("Song changed " .. song_changes .. " times, no song is currently playing")
		end
	end,
	on_state_change = function(old_state, state)
		state_changes = state_changes + 1
		log.info("State changed " .. state_changes .. " times, current state is: " .. state)
	end,
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

---@class Config
---@field address string
---@field mpris boolean
---@field on_song_change fun(old_song: Song | nil, new_song: Song | nil): nil
---@field on_state_change fun(old_state: PlaybackState, state: PlaybackState): nil

---@class Logger
---@field info fun(msg: string): nil
---@field error fun(msg: string): nil
---@field debug fun(msg: string): nil
---@field warn fun(msg: string): nil
---@field trace fun(msg: string): nil

---Global logger injected by rmpcd (Rust) into Lua as `log`.
---@type Logger
log = log
```
