local lrclib_url = "https://lrclib.net"

---@type LyricsPlugin
local M = {
    enabled = true,
    debounce_delay = 500,
    lyrics_dir = os.getenv("HOME") .. "/Music",
    checked_uris = {},
}

local function last_path_segment(s)
    local before = s:match("^(.*)/[^/]*$")
    return before or s
end

local function replace_after_last_dot(s, replacement)
    return s:gsub("%.[^.]*$", "." .. replacement, 1)
end

---@param self LyricsPlugin
---@param song Song
local function download(self, song)
    log.info("Fetching lyrics for " .. song.artist .. " - " .. song.title .. " at path " .. song.file)
    fs.create_dir_all(self.lyrics_dir .. "/" .. last_path_segment(song.file))
    local lrc_path = self.lyrics_dir .. "/" .. replace_after_last_dot(song.file, "lrc")

    if fs.exists(lrc_path) then
        log.info("Lyrics file already exists at " .. lrc_path .. ", skipping download")
        return
    end

    if song.artist == nil or song.title == nil or song.album == nil then
        log.warn("Song metadata missing artist or title, cannot fetch lyrics for " .. song.file)
        process.spawn({
            "rmpc",
            "remote",
            "status",
            "--level",
            "warn",
            "Cannot download lyrics for " .. song.file .. " due to incomplete metadata",
        })
        return
    end

    local result = http.get(lrclib_url .. "/api/get", {
        headers = {
            ["User-Agent"] = "rmpcd v0.1.0 (https://github.com/mierak/rmpc)",
        },
        params = {
            artist_name = song.artist:first(),
            track_name = song.title:first(),
            album_name = song.album:first(),
            duration = string.format("%.0f", song.duration / 1000),
        },
    })

    if result.code == 404 then
        log.info("Lyrics not found for " .. song.artist .. " - " .. song.title)
        process.spawn({
            "rmpc",
            "remote",
            "status",
            "--level",
            "warn",
            "Lyrics for '" .. song.artist .. " - " .. song.title .. "' not found",
        })
        return
    end

    if result.code ~= 200 then
        log.error("Error fetching lyrics: HTTP " .. result.code)
        process.spawn({ "rmpc", "remote", "status", "--level", "error", "Failed to download lyrics" })
        return
    end

    local json = result:json()
    if util.nil_or_null(json.syncedLyrics) or json.syncedLyrics == "" then
        log.info("Synced lyrics not found for " .. song.artist .. " - " .. song.title)
        process.spawn({
            "rmpc",
            "remote",
            "status",
            "--level",
            "warn",
            "Synced lyrics for '" .. song.artist .. " - " .. song.title .. "' not found",
        })
        return
    end

    local lrc = ""
    lrc = lrc .. "[ar:" .. song.artist .. "]\n"
    lrc = lrc .. "[al:" .. song.album .. "]\n"
    lrc = lrc .. "[ti:" .. song.title .. "]\n"
    lrc = lrc .. json.syncedLyrics
    log.info("Saving lyrics to " .. lrc_path)
    fs.write_str(lrc_path, lrc)

    process.spawn({ "rmpc", "remote", "indexlrc", "--path", lrc_path })
end

---@param _self LyricsPlugin
---@param _song Song
local function download_debounced(_self, _song) end

M.setup = function(self, args)
    self.enabled = (args.enabled ~= nil) and args.enabled or true

    if args.lyrics_dir ~= nil then
        self.lyrics_dir = args.lyrics_dir
    end

    local rmpc = util.which("rmpc")
    if not rmpc then
        log.error("rmpc not found in PATH, disabling lyrics plugin")
        self.enabled = false
    end

    local debounce_delay = args.debounce_delay or 1000
    if debounce_delay < 0 then
        download_debounced = download
    else
        download_debounced = sync.debounce(debounce_delay, download)
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

M.song_change = function(self, _old_song, new_song)
    if not self.enabled then
        return
    end

    if new_song == nil then
        return
    end

    if self.checked_uris[new_song.file] then
        log.info("Already checked for lyrics for " .. new_song.file .. ", skipping")
        return
    else
        self.checked_uris[new_song.file] = true
    end

    download_debounced(self, new_song)
end

return M
