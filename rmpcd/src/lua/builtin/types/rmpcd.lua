---@alias IdleEvent "player" | "mixer" | "options" | "playlist" | "database" | "update" | "stored_playlist" | "sticker" | "subscription" | "shelf"

---@class RmpcdGlobal
---@field install fun(path: string): RmpcPlugin
---@field install fun(path: "#builtin.lastfm"): LastFmPlugin
---@field install fun(path: "#builtin.notify"): NotifyPlugin
---@field install fun(path: "#builtin.playcount"): PlaycountPlugin
---@field install fun(path: "#builtin.lyrics"): LyricsPlugin

---@class RmpcPlugin<Args>
---@field setup fun(self: RmpcPlugin<Args>, args: Args) | nil
---@field song_change fun(self: RmpcPlugin<Args>, old_song: Song | nil, new_song: Song | nil) | nil
---@field state_change fun(self: RmpcPlugin<Args>, old_state: PlaybackState, new_state: PlaybackState) | nil
---@field messages fun(self: RmpcPlugin<Args>, messages: table<string, string[]>) | nil
---@field message fun(self: RmpcPlugin<Args>, channel: string, messages: string) | nil
---@field idle_event fun(self: RmpcPlugin<Args>, event: IdleEvent) | nil
---@field shutdown fun(self: RmpcPlugin<Args>) | nil

---@type RmpcdGlobal
---@diagnostic disable-next-line: lowercase-global
rmpcd = rmpcd
