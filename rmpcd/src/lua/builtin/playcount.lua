local log = require("rmpcd.log")
local mpd = require("rmpcd.mpd")

local function playcount(old_song, new_song)
    local sticker, err = mpd.get_song_sticker(new_song.file, "playcount")
    if err then
        log.error("Error retrieving playcount sticker for '%s': %s", new_song.file, err)
        return
    end

    if sticker == nil then
        mpd.set_song_sticker(new_song.file, "playcount", "1")
    else
        local count = tonumber(sticker) or 0
        mpd.set_song_sticker(new_song.file, "playcount", tostring(count + 1))
    end
end

return {
    install = function()
        rmpcd.on("song_change", playcount)
    end,
}
