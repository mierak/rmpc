local lrclib_url = "https://lrclib.net"
local request_timeout_ms = 10000

---@type LyricsPlugin
local M = {
    enabled = true,
    debounce_delay = 500,
    lyrics_dir = os.getenv("HOME") .. "/Music",
    checked_uris = {},
}

local function parent_dir(path)
    local parent = path:match("^(.*)/[^/]*$")
    if parent == nil then
        return "."
    end
    if parent == "" then
        return "/"
    end
    return parent
end

local function replace_after_last_dot(s, replacement)
    return s:gsub("%.[^.]*$", "." .. replacement, 1)
end

---@param self LyricsPlugin
---@param song Song
local function download(self, song)
    local lrc_path = self.lyrics_dir .. "/" .. replace_after_last_dot(song.file, "lrc")

    if fs.exists(lrc_path) then
        log.info("Lyrics file already exists at " .. lrc_path .. ", skipping download")
        return
    end

    if song.artist == nil or song.title == nil then
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

    log.info("Fetching lyrics for " .. song.artist .. " - " .. song.title .. " at path " .. song.file)
    local result = http.get(lrclib_url .. "/api/get", {
        timeout_ms = request_timeout_ms,
        headers = {
            ["User-Agent"] = "rmpcd v0.1.0 (https://github.com/mierak/rmpc)",
        },
        params = {
            artist_name = song.artist:first(),
            track_name = song.title:first(),
            album_name = song.album and song.album:first() or nil,
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

    if result.code ~= 200 or result.error ~= nil then
        log.error("Error fetching lyrics: " .. (result.error or ("HTTP " .. tostring(result.code))))
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
    if song.album ~= nil then
        lrc = lrc .. "[al:" .. song.album .. "]\n"
    end
    lrc = lrc .. "[ti:" .. song.title .. "]\n"
    lrc = lrc .. json.syncedLyrics
    log.info("Saving lyrics to " .. lrc_path)

    local _, err = fs.create_dir_all(parent_dir(lrc_path))
    if err == nil then
        _, err = fs.write_str(lrc_path, lrc)
    end

    if err ~= nil then
        log.error("Failed to save lyrics to '" .. lrc_path .. "': " .. tostring(err))
        process.spawn({ "rmpc", "remote", "status", "--level", "error", "Failed to save lyrics" })
        return
    end

    process.spawn({ "rmpc", "remote", "indexlrc", "--path", lrc_path })
end

---@param _self LyricsPlugin
---@param _song Song
local function download_debounced(_self, _song) end

M.setup = function(self, args)
    self.enabled = args.enabled == nil or args.enabled

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
