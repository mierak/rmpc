---@type PlaycountPlugin
local M = {
    enabled = true,
}

M.setup = function(_self, args)
    _self.enabled = (args.enabled ~= nil) and args.enabled or true
end

M.subscribed_channels = { "rmpcd.playcount" }
M.message = function(self, _channel, message)
    if message == "enable" then
        log.info("Enabling playcount plugin")
        self.enabled = true
    elseif message == "disable" then
        log.info("Disabling playcount plugin")
        self.enabled = false
    elseif message == "toggle" then
        log.info("Toggling notify playcount to: " .. tostring(not self.enabled))
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

    local sticker, err = mpd.get_song_sticker(new_song.file, "playcount")
    if err then
        log.error("Error retrieving playcount sticker for " .. new_song.file)
        return
    end

    if sticker == nil then
        mpd.set_song_sticker(new_song.file, "playcount", "1")
    else
        local count = tonumber(sticker) or 0
        mpd.set_song_sticker(new_song.file, "playcount", tostring(count + 1))
    end
end

return M
