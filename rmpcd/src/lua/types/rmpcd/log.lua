---@meta

---@class Log
---@field info fun(msg: string)
---@field error fun(msg: string)
---@field debug fun(msg: string)
---@field warn fun(msg: string)
---@field trace fun(msg: string)

---@class _G
---@field log Log

local log = {}
_G.log = log
return log
