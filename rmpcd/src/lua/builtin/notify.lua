---@type NotifyPlugin
local M = {
    enabled = true,
    with_album_art = true,
    album_art_path = "/tmp/rmpcd-notify-album-art",
}

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

M.setup = function(self, args)
    self.with_album_art = (args.with_album_art ~= nil) and args.with_album_art or true
    self.album_art_path = args.album_art_path or "/tmp/rmpcd-notify-album-art"
    self.enabled = (args.enabled ~= nil) and args.enabled or true

    local notify_send = util.which("notify-send")
    if not notify_send then
        log.error("notify-send not found in PATH, disabling notify plugin")
        self.enabled = false
    end
end

M.song_change = function(self, _old_song, new_song)
    if not self.enabled then
        return
    end

    if new_song == nil then
        return
    end

    notify(new_song, self.with_album_art or true, self.album_art_path)
end

M.subscribed_channels = { "rmpcd.notify" }
M.message = function(self, _channel, message)
    if message == "enable" then
        log.info("Enabling notify plugin")
        self.enabled = true
    elseif message == "disable" then
        log.info("Disabling notify plugin")
        self.enabled = false
    elseif message == "toggle" then
        log.info("Toggling notify plugin to: " .. tostring(not self.enabled))
        self.enabled = not self.enabled
    end
end

return M
