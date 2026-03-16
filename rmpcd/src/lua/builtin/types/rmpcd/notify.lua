---@meta
---@module "rmpcd.notify"

---@class NotifyArgs
---@field enabled? boolean
---@field with_album_art? boolean
---@field album_art_path? string
---@field debounce_delay? integer

---@class NotifyPlugin: RmpcdPlugin<NotifyArgs>
---@field enabled boolean
---@field with_album_art boolean
---@field album_art_path string

local M = {}
return M
