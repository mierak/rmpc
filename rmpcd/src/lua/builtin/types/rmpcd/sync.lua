---@meta

---@class TimeoutHandle
---@field cancel fun()

---@class Sync
-- ---@field set_timeout fun(timeout_ms: integer, callback: fun()): TimeoutHandle
-- ---@field set_interval fun(interval_ms: integer, callback: fun(handle: TimeoutHandle)): TimeoutHandle
-- ---@field debounce fun(timeout_ms: integer, callback: fun(...: any)): fun(...: any)

---@type Sync
---@diagnostic disable-next-line: lowercase-global

---@class _G
---@field sync Sync

local sync = {}
_G.sync = sync
return sync
