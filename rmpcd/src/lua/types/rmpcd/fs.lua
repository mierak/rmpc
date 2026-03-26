---@meta

---@class Fs
---@field exists fun(path: string): (boolean, string|nil)
---@field create_dir_all fun(path: string): (boolean, string|nil)
---@field create_dir fun(path: string): (boolean, string|nil)
---@field write fun(path: string, contents: integer[]): (boolean, string|nil)
---@field write_str fun(path: string, contents: string): (boolean, string|nil)
---@field read fun(path: string): (integer[]|nil, string|nil)
---@field read_str fun(path: string): (string|nil, string|nil)
---@field delete fun(path: string): (boolean, string|nil)
---@field remove_dir fun(path: string): (boolean, string|nil)
---@field remove_dir_all fun(path: string): (boolean, string|nil)

---@class _G
---@field fs Fs

local fs = {}
_G.fs = fs
return fs
