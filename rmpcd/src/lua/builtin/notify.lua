local process = require("rmpcd.process")
local mpd = require("rmpcd.mpd")
local fs = require("rmpcd.fs")

---@class NotifyPlugin
local M = {}

---@param new_song Song
---@param with_album_art boolean
---@param album_art_path string
local function notify(new_song, with_album_art, album_art_path)
    local artist = (new_song.artist and new_song.artist:first()) or "Unknown Artist"
    local title = (new_song.title and new_song.title:first()) or "Unknown Title"

    if not with_album_art then
        process.spawn({ "notify-send", "Now playing: " .. artist .. " - " .. title })
        return
    end

    local bytes, err = mpd.read_picture(new_song.file)
    if err ~= nil then
        bytes, err = mpd.album_art(new_song.file)
    end

    if err ~= nil or bytes == nil then
        process.spawn({ "notify-send", "Now playing: " .. artist .. " - " .. title })
    else
        fs.write(album_art_path, bytes)
        process.spawn({ "notify-send", "-i", album_art_path, "Now playing: " .. artist .. " - " .. title })
    end
end

---@param args { with_album_art: boolean | nil, album_art_path: string | nil } | nil
M.setup = function(self, args)
    local _args = args or {}
    self.with_album_art = _args.with_album_art or true
    self.album_art_path = _args.album_art_path or "/tmp/rmpcd-notify-album-art"
end

M.song_change = function(self, _old_song, new_song)
    if new_song == nil then
        return
    end

    notify(new_song, self.with_album_art or true, self.album_art_path)
end

return M
