---@meta

---@class Util
---@field dump_table fun(tbl: table)
---@field md5 fun(data: string): string
---@field which fun(prog: string): boolean

---@type Util
---@diagnostic disable-next-line: lowercase-global

---@class _G
---@field util Util

local util = {}
_G.util = util
return util
