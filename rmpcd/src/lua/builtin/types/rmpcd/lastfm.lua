---@meta
---@module "rmpcd.lastfm"

---@class LastFmArgs
---@field api_key string For now you are required to create your own LastFM api key because of their insane API design https://www.last.fm/api/account/create
---@field shared_secret string For now you are required to create your own LastFM secret because of their insane API design https://www.last.fm/api/account/create
---@field update_now_playing? boolean Whether to update the now playing status on song change

---@class LastFmPlugin: RmpcPlugin<LastFmArgs>
---@field session_key string | nil
---@field scrobble_queue Deque<{ song: Song, timestamp: integer }>
---@field song_start integer | nil
---@field current_song Song | nil
---@field api_key string
---@field shared_secret string
---@field update_now_playing boolean

local M = {}
return M
