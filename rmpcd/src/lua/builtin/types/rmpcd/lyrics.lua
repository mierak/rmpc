---@meta
---@module "rmpcd.lyrics"

---@class LyricsArgs
---@field enabled? boolean
---@field debounce_delay? integer
---@field lyrics_dir string?

---@class LyricsPlugin: RmpcdPlugin<LyricsArgs>
---@field enabled boolean
---@field lyrics_dir string
---@field checked_uris table<string, boolean>

local M = {}
return M
