local sync = require("rmpcd.sync")
local process = require("rmpcd.process")
local mpd = require("rmpcd.mpd")
local fs = require("rmpcd.fs")

---@param new_song Song
---@param with_album_art boolean
---@param album_art_path string
local function notify(new_song, with_album_art, album_art_path)
    local artist
    if new_song.artist and type(new_song.artist) == "table" then
        artist = new_song.artist[1]
    elseif new_song.artist and type(new_song.artist) == "string" then
        artist = new_song.artist
    else
        artist = "Unknown Artist"
    end

    local title
    if new_song.title and type(new_song.title) == "table" then
        title = new_song.title[1]
    elseif new_song.title and type(new_song.title) == "string" then
        title = new_song.title
    else
        title = "Unknown Title"
    end

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

local album_art_path = "/tmp/rmpcd-notify-album-art"
---@type NotifyModule
return {
    install = function(args)
        local _args = args or {}

        local debounced = sync.debounce(500, function(_old_song, new_song)
            if new_song == nil then
                return
            end

            notify(new_song, _args.with_album_art or true, _args.album_art_path or album_art_path)
        end)

        rmpcd.on("song_change", debounced)
    end,
}
