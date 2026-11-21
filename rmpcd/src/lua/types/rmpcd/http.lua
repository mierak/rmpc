---@meta

---@alias HttpMethod
---| "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS" | "CONNECT" | "TRACE"

---@class HttpRequestOpts
---@field method? HttpMethod
---@field headers? table<string, string>
---@field body? string
---@field params? table<string, string>
---@field timeout_ms? integer

---@class HttpGetOpts
---@field headers? table<string, string>
---@field params? table<string, string>
---@field timeout_ms? integer

---@class HttpPostOpts
---@field headers? table<string, string>
---@field body? string
---@field params? table<string, string>
---@field timeout_ms? integer

---@class HttpResponse
---@field code integer|nil
---@field error string|nil
---@field body string|nil
---@field json fun(self: HttpResponse): (any|nil) Decoded body, nil if the request failed before a body was read. Raises if the body is not valid JSON.
---@field text fun(self: HttpResponse): (string|nil) The body, nil if the request failed before a body was read.

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
