local sync = require("rmpcd.sync")
local process = require("rmpcd.process")

local function notify(new_song)
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

    process.spawn({ "notify-send", "Now playing: " .. artist .. " - " .. title })
end

return {
    install = function()
        local debounced = sync.debounce(500, function(_old_song, new_song)
            notify(new_song)
        end)

        rmpcd.on("song_change", debounced)
    end,
}
