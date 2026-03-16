---@meta

---@class TimeoutHandle
---@field cancel fun()

---@class Sync
---@field set_timeout fun(timeout_ms: integer, callback: fun()): TimeoutHandle
---@field set_interval fun(interval_ms: integer, callback: fun(handle: TimeoutHandle)): TimeoutHandle

---@type Sync
---@diagnostic disable-next-line: lowercase-global

---@class _G
---@field sync Sync

---@generic T
---@param interval_ms integer
---@param callback T
---@return T
---@diagnostic disable-next-line: inject-field
function _G.sync.debounce(interval_ms, callback) end

local sync = {}
_G.sync = sync
return sync
