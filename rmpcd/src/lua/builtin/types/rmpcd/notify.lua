---@meta
---@module "rmpcd.notify"

---@class NotifyArgs
---@field enabled? boolean
---@field with_album_art? boolean
---@field album_art_path? string

---@class NotifyPlugin: RmpcPlugin<NotifyArgs>
---@field enabled boolean
---@field with_album_art boolean
---@field album_art_path string

local M = {}
return M
