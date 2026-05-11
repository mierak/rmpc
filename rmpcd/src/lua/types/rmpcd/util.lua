---@meta

---@class Util
---@field dump_table fun(tbl: table)
---@field md5 fun(data: string): string
---@field which fun(prog: string): boolean
---@field nil_or_null fun(value: any): boolean
---@field deserialize_ron fun(value: number[]): (any | nil, string | nil)

---@type Util
---@diagnostic disable-next-line: lowercase-global

---@class _G
---@field util Util

local util = {}
_G.util = util
return util
