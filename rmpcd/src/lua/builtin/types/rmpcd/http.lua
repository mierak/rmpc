---@meta

---@alias HttpMethod
---| "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS" | "CONNECT" | "TRACE"

---@class HttpRequestOpts
---@field method? HttpMethod
---@field headers? table<string, string>
---@field body? string
---@field params? table<string, string>

---@class HttpGetOpts
---@field headers? table<string, string>
---@field params? table<string, string>

---@class HttpPostOpts
---@field headers? table<string, string>
---@field body? string
---@field params? table<string, string>

---@class HttpResponse
---@field code integer|nil
---@field error string|nil
---@field body string|nil
---@field json fun(self: HttpResponse): any
---@field text fun(self: HttpResponse): string

---@class RmpcdHttp
---@field call fun(url: string, opts?: HttpRequestOpts): HttpResponse
---@field get fun(url: string, opts?: HttpGetOpts): HttpResponse
---@field post fun(url: string, opts?: HttpPostOpts): HttpResponse

---@type RmpcdHttp
---@diagnostic disable-next-line: lowercase-global

---@class _G
---@field http RmpcdHttp

local http = {}
_G.http = http
return http
