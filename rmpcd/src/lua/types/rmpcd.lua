---@alias IdleEvent "player" | "mixer" | "options" | "playlist" | "database" | "update" | "stored_playlist" | "sticker" | "subscription" | "shelf"

---@class RmpcdGlobal
---@field install fun(path: string): RmpcdPlugin
---@field install fun(path: "#builtin.lastfm"): LastFmPlugin
---@field install fun(path: "#builtin.notify"): NotifyPlugin
---@field install fun(path: "#builtin.playcount"): PlaycountPlugin
---@field install fun(path: "#builtin.lyrics"): LyricsPlugin

---@generic Args
---@class RmpcdPlugin<Args>
---@field subscribed_channels string[]|nil
---@field setup fun(self, args: Args)|nil
---@field song_change fun(self, old_song: Song|nil, new_song: Song|nil)|nil
---@field state_change fun(self, old_state: PlaybackState, new_state: PlaybackState)|nil
---@field message fun(self, channel: string, message: string)|nil
---@field idle_event fun(self, event: IdleEvent)|nil
---@field shutdown fun(self)|nil
---@field reconnect fun(self)|nil

---@type RmpcdGlobal
---@diagnostic disable-next-line: lowercase-global
rmpcd = rmpcd

---@class Config
---@field address string Point to your mpd server, e.g. "localhost:6600".
---@field password string | nil Password for your mpd server, if any.
---@field mpris boolean | nil
---@field subscribed_channels string[] | nil
