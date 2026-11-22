-- ##############################################################
-- ##                                                          ##
-- ## This is an example config file. Uncomment and customize  ##
-- ## to your liking. Check out the documentation for more     ##
-- ## information. https://rmpc.mierak.dev/rmpcd/              ##
-- ##                                                          ##
-- ##############################################################

---@type Config
local config = {
    -- Point rmpcd to your mpd server
    address = "127.0.0.1:6600",
}

-- Enable mpris support
-- config.mpris = true

-- Prevent screen lock and idle suspend while MPD is playing
-- config.inhibit_idle = true

-- Disable TCP keepalive on the MPD socket (enabled by default)
-- config.enable_keepalive = false

-- Automatically increment play count on song change
-- Also see https://github.com/rmpc-org/rmpcd-playcount
-- rmpcd.install({ url = "https://github.com/rmpc-org/rmpcd-playcount.git" })

-- Install last fm scrobbling plugin
-- For now you have to request an API key yourself due to Last.fm's insane API
-- design https://www.last.fm/api/account/create
-- Also see https://github.com/rmpc-org/rmpcd-lastfm
-- ---@type LastFmPluginHandle
-- local lastfm = rmpcd.install({ url = "https://github.com/rmpc-org/rmpcd-lastfm.git" })
-- lastfm:setup({
--     api_key = "<your api key>",
--     shared_secret = "<your shared secret>",
-- })

-- Install notification on song change plugin
-- Also see https://github.com/rmpc-org/rmpcd-notify
-- rmpcd.install({ url = "https://github.com/rmpc-org/rmpcd-notify.git" })

-- Install the auto lyrics download plugin
-- Also see https://github.com/rmpc-org/rmpcd-lyrics
-- rmpcd.install({ url = "https://github.com/rmpc-org/rmpcd-lyrics.git" })

-- Install the last played time tracking plugin
-- Also see https://github.com/rmpc-org/rmpcd-lastplayed
-- rmpcd.install({ url = "https://github.com/rmpc-org/rmpcd-lastplayed.git" })

return config
