---@meta
---@module "rmpcd.mpd"

---@class Mpd
---@field set_song_sticker fun(uri: string, name: string, value: string): (boolean, string|nil)
---@field get_song_sticker fun(uri: string, name: string): (string|nil, string|nil)
---@field set_consume fun(value: OnOffOneshot): (boolean, string|nil)
---@field set_crossfade fun(seconds: integer): (boolean, string|nil)
---@field set_random fun(value: boolean): (boolean, string|nil)
---@field set_repeat fun(value: boolean): (boolean, string|nil)
---@field set_single fun(value: OnOffOneshot): (boolean, string|nil)
---@field get_volume fun(): (integer|nil, string|nil)
---@field set_volume fun(volume: integer): (boolean, string|nil)
---@field volume fun(value: ValueChange): (boolean, string|nil)
---@field prev fun(): (boolean, string|nil)
---@field next fun(): (boolean, string|nil)
---@field seek_current fun(value: ValueChange): (boolean, string|nil)
---@field play fun(): (boolean, string|nil)
---@field pause fun(): (boolean, string|nil)
---@field toggle_pause fun(): (boolean, string|nil)
---@field stop fun(): (boolean, string|nil)
---@field get_status fun(): (MpdStatus|nil, nil|string)
---@field album_art fun(uri: string): (integer[]|nil, string|nil)
---@field read_picture fun(uri: string): (integer[]|nil, string|nil)
---@field subscribe fun(channel: string): (boolean, string|nil)
---@field unsubscribe fun(channel: string): (boolean, string|nil)
---@field channels fun(): (string[]|nil, string|nil)
---@field send_message fun(channel: string, message: string): (boolean, string|nil)
---@field read_messages fun(): (table<string, string[]>|nil, string|nil)

---@alias ValueChange string e.g. "+5", "-10", "50"

---@class _G
---@field mpd Mpd

local mpd = {}
_G.mpd = mpd
return mpd
