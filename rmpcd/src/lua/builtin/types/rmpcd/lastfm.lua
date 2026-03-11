---@meta
---@module "rmpcd.lastfm"

---@class LastFmArgs
---@field api_key string For now you are required to create your own LastFM api key because of their insane API design https://www.last.fm/api/account/create
---@field shared_secret string For now you are required to create your own LastFM secret because of their insane API design https://www.last.fm/api/account/create

---@class LastFmModule
---@field install fun(args: LastFmArgs)
local M = {}
return M
