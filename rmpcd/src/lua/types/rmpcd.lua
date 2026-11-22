---@alias IdleEvent "player" | "mixer" | "options" | "playlist" | "database" | "update" | "stored_playlist" | "sticker" | "subscription" | "shelf"

---@class RmpcdGlobal
---@field install fun(path: string): RmpcdPluginHandle<table>
---@field install fun(path: "#builtin.lastfm"): RmpcdPluginHandle<LastFmArgs>
---@field install fun(path: "#builtin.notify"): RmpcdPluginHandle<NotifyArgs>
---@field install fun(path: "#builtin.playcount"): RmpcdPluginHandle<{ enabled?: boolean }>
---@field install fun(path: "#builtin.lyrics"): RmpcdPluginHandle<LyricsArgs>
---@field install fun(spec: RmpcdPluginSpec): RmpcdPluginHandle<table>

---@class RmpcdPluginSpec
---@field url string
---@field branch string | nil

---@generic Args
---@class RmpcdPluginHandle<Args>
---@field setup fun(self, args: Args)

---@generic Args
---@class RmpcdPlugin<Args>
---@field subscribed_channels string[]|nil
---@field setup fun(self, args: Args)|nil
---@field song_change fun(self, old_song: QueuedSong|nil, new_song: QueuedSong|nil)|nil
---@field state_change fun(self, old_state: PlaybackState, new_state: PlaybackState)|nil
---@field message fun(self, channel: string, message: string)|nil
---@field idle_event fun(self, event: IdleEvent)|nil
---@field start fun(self)|nil Called exactly once after rmpcd has connected to MPD and all plugins are loaded. Not called again on reconnect.
---@field shutdown fun(self)|nil
---@field reconnect fun(self)|nil Called after the connection to MPD is re-established. Channels from `subscribed_channels` are resubscribed before this runs.

---@type RmpcdGlobal
---@diagnostic disable-next-line: lowercase-global
rmpcd = rmpcd

---@class Config
---@field address string Point to your mpd server, e.g. "localhost:6600".
---@field password string | nil Password for your mpd server, if any.
---@field mpris boolean | nil
---@field mpris_identity string | nil Player name shown by MPRIS clients. Defaults to "Music Player Daemon", or "Music Player Daemon (<instance>)" when started with `--instance`.
---@field inhibit_idle boolean | nil Prevent the system from going idle while MPD is playing.
---@field subscribed_channels string[] | nil
