---@meta

---@class Process
---@field spawn fun(cmd: string[]): (integer|nil, string|nil)
---@field pid fun(): integer PID of the rmpcd process

---@type Process
---@diagnostic disable-next-line: lowercase-global

---@class _G
---@field process Process

local process = {}
_G.process = process
return process
