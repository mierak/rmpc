---@alias RmpcdHookName
---| "song_change"
---| "state_change"
---| "messages"
---| "message"
---| "idle_event"

---@class RmpcdGlobal
---@field hooks table<RmpcdHookName, table>
---@field on fun(hook: RmpcdHookName, callback: function)

---@type RmpcdGlobal
---@diagnostic disable-next-line: lowercase-global
rmpcd = rmpcd
