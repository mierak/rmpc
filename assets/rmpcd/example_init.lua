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

-- Automatically increment play count on song change
-- rmpcd.install("#builtin.playcount")

-- Install last fm scrobbling builtin
-- For now you have to request an API key yourself due to LastFM's insane API
-- design https://www.last.fm/api/account/create
-- rmpcd.install("#builtin.lastfm"):setup({
-- 	api_key = "<your api key>",
-- 	shared_secret = "<your shared secret key>",
-- 	update_now_playing = false,
-- })

-- Install notification on song change builtin
-- rmpcd.install("#builtin.notify"):setup({
-- 	debounce_delay = 1000,
-- })

-- Install the auto lyrics download builtin
-- rmpcd.install("#builtin.lyrics"):setup({
-- 	debounce_delay = 1000,
-- })

return config
