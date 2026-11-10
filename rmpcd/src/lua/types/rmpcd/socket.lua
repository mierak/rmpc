---@meta

---@class UnixSocketHandlers
---@field on_data fun(data: string) Receives raw bytes as a lua string.
---@field on_close fun(reason: string | nil) | nil
---@field write_timeout_ms integer|nil How long a single `write` may take. Default 5000. The socket is closed on timeout.

---@class UnixSocket
---@field write fun(self: UnixSocket, data: string): (boolean, string | nil) Writes all of `data`. Returns false, "closed" if the socket is closed, false, "timeout" if the peer did not read in time.
---@field close fun(self: UnixSocket) Closes the socket. You should always close any sockets you create this way.

---@class Socket
---@field connect_unix fun(path: string, handlers: UnixSocketHandlers): (UnixSocket | nil, string | nil) Connects to a unix domain socket.

---@type Socket
---@diagnostic disable-next-line: lowercase-global

---@class _G
---@field socket Socket

local socket = {}
_G.socket = socket
return socket
