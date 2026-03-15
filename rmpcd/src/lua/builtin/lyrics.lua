local lrclib_url = "https://lrclib.net"
local lyrics_dir = os.getenv("HOME") .. "/Music"

---@type LyricsPlugin
local M = {
    enabled = true,
}

local function last_path_segment(s)
    local before = s:match("^(.*)/[^/]*$")
    return before or s
end

local function replace_after_last_dot(s, replacement)
    return s:gsub("%.[^.]*$", "." .. replacement, 1)
end

M.setup = function(self, args)
    self.enabled = (args.enabled ~= nil) and args.enabled or true

    local rmpc = util.which("rmpc")
    if not rmpc then
        log.error("rmpc not found in PATH, disabling lyrics plugin")
        self.enabled = false
    end
end

M.subscribed_channels = { "rmpcd.lyrics" }
M.message = function(self, _channel, message)
    if message == "enable" then
        log.info("Enabling lyrics plugin")
        self.enabled = true
    elseif message == "disable" then
        log.info("Disabling lyrics plugin")
        self.enabled = false
    elseif message == "toggle" then
        log.info("Toggling lyrics plugin to: " .. tostring(not self.enabled))
        self.enabled = not self.enabled
    end
end

M.song_change = function(_self, _old_song, new_song)
    if not _self.enabled then
        return
    end

    if new_song == nil then
        return
    end

    log.info("Fetching lyrics for " .. new_song.artist .. " - " .. new_song.title .. " at path " .. new_song.file)
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
        process.spawn({
            "rmpc",
            "remote",
            "status",
            "--level",
            "warn",
            "Lyrics for '" .. new_song.artist .. " - " .. new_song.title .. "' not found",
        })
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
    fs.write_str(lrc_path, lrc)

    process.spawn({ "rmpc", "remote", "indexlrc", "--path", lrc_path })
end

return M
