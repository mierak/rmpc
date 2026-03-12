---@alias RmpcdHookName
---| "song_change"
---| "state_change"
---| "messages"
---| "message"
---| "idle_event"

---@alias IdleEvent "player" | "mixer" | "options" | "playlist" | "database" | "update" | "stored_playlist" | "sticker" | "subscription" | "shelf"

---@alias song_change_callback fun(old_song: Song | nil, new_song: Song | nil)
---@alias state_change_callback fun(old_state: PlaybackState, new_state: PlaybackState)
---@alias messages_callback fun(messages: table<string, string[]>)
---@alias message_callback fun(channel: string, messages: string[])
---@alias idle_event_callback fun(event: IdleEvent)

---@class RmpcdGlobal
---@field hooks table<RmpcdHookName, table>
---@field on fun(hook: "song_change", callback: song_change_callback) | fun(hook: "state_change", callback: state_change_callback) | fun(hook: "messages", callback: messages_callback) | fun(hook: "message", callback: message_callback) | fun(hook: "idle_event", callback: idle_event_callback)

---@type RmpcdGlobal
---@diagnostic disable-next-line: lowercase-global
rmpcd = rmpcd
