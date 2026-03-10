---@meta
---@module "rmpcd.log"

---@class Log
---@field info fun(msg: string)
---@field error fun(msg: string)
---@field debug fun(msg: string)
---@field warn fun(msg: string)
---@field trace fun(msg: string)

---@type Log
---@diagnostic disable-next-line: missing-fields
local M = {}
return M
