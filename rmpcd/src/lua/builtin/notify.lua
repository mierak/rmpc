local sync = require("rmpcd.sync")
local process = require("rmpcd.process")
local mpd = require("rmpcd.mpd")
local fs = require("rmpcd.fs")
local log = require("rmpcd.log")

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

local album_art_path = "/tmp/rmpcd-notify-album-art"
---@type NotifyModule
return {
    install = function(args)
        local _args = args or {}

        local debounced = sync.debounce(
            500,
            ---@param _old_song Song | nil
            ---@param new_song Song | nil
            ---@diagnostic disable-next-line: unused-local
            function(_old_song, new_song)
                log.info("Notifying")
                if new_song == nil then
                    log.info("No new song, skipping notification")
                    return
                end

                notify(new_song, _args.with_album_art or true, _args.album_art_path or album_art_path)
                log.info("Notification sent")
            end
        )

        rmpcd.on("song_change", debounced)
        -- rmpcd.on("song_change", function(_old_song, new_song)
        --     if new_song == nil then
        --         return
        --     end
        --
        --     notify(new_song, _args.with_album_art or true, _args.album_art_path or album_art_path)
        -- end)
    end,
}
