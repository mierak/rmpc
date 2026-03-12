local sync = require("rmpcd.sync")
local fs = require("rmpcd.fs")
local http = require("rmpcd.http")
local log = require("rmpcd.log")
local process = require("rmpcd.process")

local lrclib_url = "https://lrclib.net"
local lyrics_dir = os.getenv("HOME") .. "/Music"

local function last_path_segment(s)
    local before = s:match("^(.*)/[^/]*$")
    return before or s
end

local function replace_after_last_dot(s, replacement)
    return s:gsub("%.[^.]*$", "." .. replacement, 1)
end

---@type LyricsModule
return {
    install = function()
        local debounced = sync.debounce(
            500,
            ---@param _old_song Song | nil
            ---@param new_song Song | nil
            ---@diagnostic disable-next-line: unused-local
            function(_old_song, new_song)
                if new_song == nil then
                    return
                end

                log.info(
                    "Fetching lyrics for " .. new_song.artist .. " - " .. new_song.title .. " at path " .. new_song.file
                )
                fs.create_dir_all(lyrics_dir .. "/" .. last_path_segment(new_song.file))
                local lrc_path = lyrics_dir .. "/" .. replace_after_last_dot(new_song.file, "lrc")

                if fs.exists(lrc_path) then
                    log.info("Lyrics file already exists at " .. lrc_path .. ", skipping download")
                    return
                end

                local result = http.get(lrclib_url .. "/api/get", {
                    headers = {
                        ["Lrclib-Client"] = "rmpcd-0.1.0",
                    },
                    params = {
                        artist_name = new_song.artist:first(),
                        track_name = new_song.title:first(),
                        album_name = new_song.album:first(),
                    },
                })

                if result.code == 404 then
                    process.spawn({ "rmpc", "remote", "status", "--level", "warn", "Lyrics not found" })
                    return
                end

                if result.code ~= 200 then
                    log.error("Error fetching lyrics: HTTP " .. result.code)
                    process.spawn({ "rmpc", "remote", "status", "--level", "error", "Failed to download lyrics" })
                    return
                end

                local json = result:json()
                if json.syncedLyrics == nil or json.syncedLyrics == "null" or json.syncedLyrics == "" then
                    return
                end

                local lrc = ""
                lrc = lrc .. "[ar:" .. new_song.artist .. "]\n"
                lrc = lrc .. "[al:" .. new_song.album .. "]\n"
                lrc = lrc .. "[ti:" .. new_song.title .. "]\n"
                lrc = lrc .. json.syncedLyrics
                log.info("Saving lyrics to " .. lrc_path)
                log.debug("Lyrics content:\n" .. lrc)
                fs.write_str(lrc_path, lrc)

                process.spawn({ "rmpc", "remote", "indexlrc", "--path", lrc_path })
            end
        )

        rmpcd.on("song_change", debounced)
    end,
}
