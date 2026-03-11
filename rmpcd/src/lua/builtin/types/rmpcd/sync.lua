---@meta
---@module "rmpcd.sync"

---@class TimeoutHandle
---@field cancel fun()

---@class Sync
---@field set_timeout fun(timeout_ms: integer, callback: fun()): TimeoutHandle
---@field set_interval fun(interval_ms: integer, callback: fun(handle: TimeoutHandle)): TimeoutHandle
---@field debounce fun(timeout_ms: integer, callback: fun(...: any)): fun(...: any)

---@type Sync
---@diagnostic disable-next-line: missing-fields
local M = {}
return M
