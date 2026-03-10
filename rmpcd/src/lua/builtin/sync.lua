local sync = require("rmpcd.sync")

function sync.debounce(timeout_ms, callback)
    local token = nil

    return function(...)
        if token and token.cancel then
            token.cancel()
        end

        local args = { ... }
        local n = select("#", ...)

        token = sync.set_timeout(timeout_ms, function()
            local unpack_fn = table.unpack or unpack
            callback(unpack_fn(args, 1, n))
        end)
    end
end
