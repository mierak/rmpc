local util = require("rmpcd.util")
local http = require("rmpcd.http")
local log = require("rmpcd.log")
local process = require("rmpcd.process")
local sync = require("rmpcd.sync")

local function lastfm_api_sig(params, shared_secret)
    local keys = {}
    for k, _ in pairs(params) do
        if k ~= "format" and k ~= "callback" then
            table.insert(keys, k)
        end
    end
    table.sort(keys)

    local base = ""
    for _, k in ipairs(keys) do
        base = base .. k .. params[k]
    end
    base = base .. shared_secret

    return util.md5(base)
end

---@param apy_key string
---@param shared_secret string
---@param session_key string
---@param artist string|string[]
---@param track string|string[]
---@param album string|string[]
---@param duration_seconds number
local function lastfm_update_now_playing(apy_key, shared_secret, session_key, artist, track, album, duration_seconds)
    local params = {
        method = "track.updateNowPlaying",
        api_key = apy_key,
        sk = session_key,
        artist = artist,
        track = track,
        format = "json",
    }

    if album and album ~= "" then
        params.album = album
    end
    if duration_seconds and duration_seconds > 0 then
        params.duration = tostring(duration_seconds)
    end

    params.api_sig = lastfm_api_sig(params, shared_secret)

    local resp = http.post("https://ws.audioscrobbler.com/2.0/", {
        headers = {},
        body = nil,
        params = params,
    })

    if resp.code ~= 200 then
        log.error("Last.fm updateNowPlaying failed: HTTP " .. resp.code)
    end
end

---@param api_key string
---@param shared_secret string
---@param token string
---@return string|nil session_key
local function get_session_key(api_key, shared_secret, token)
    log.info("Waiting for browser auth...")
    local params = {
        api_key = api_key,
        method = "auth.getSession",
        token = token,
        format = "json",
    }

    params.api_sig = lastfm_api_sig(params, shared_secret)

    local session_response = http.get("https://ws.audioscrobbler.com/2.0/", { params = params })
    util.dump_table(session_response)

    if session_response.code == 200 then
        return session_response:json().session.key
    end

    return nil
end

---@type LastFmModule
return {
    install = function(args)
        local response = http.get("https://ws.audioscrobbler.com/2.0", {
            params = {
                method = "auth.getToken",
                api_key = args.api_key,
                format = "json",
            },
        })

        if response.code ~= 200 then
            log.error("Failed to get Last.fm auth token: HTTP " .. response.code)
            return
        end

        local token = response:json().token
        local session_key

        process.spawn({ "xdg-open", "https://www.last.fm/api/auth/?api_key=" .. args.api_key .. "&token=" .. token })

        ---@param sk string
        ---@param old_song Song
        ---@param song_start integer
        local function scrobble(sk, old_song, song_start)
            local params = {
                method = "track.scrobble",
                api_key = args.api_key,
                sk = sk,
                artist = old_song.artist,
                track = old_song.title,
                album = old_song.album,
                timestamp = tostring(song_start),
                format = "json",
            }

            params.api_sig = lastfm_api_sig(params, args.shared_secret)
            local resp = http.post("https://ws.audioscrobbler.com/2.0/", {
                params = params,
            })

            if resp.code ~= 200 then
                log.error("Last.fm scrobble failed: HTTP " .. resp.code)
            end
        end

        ---@param sk string
        local function register(sk)
            local song_start
            local current_song

            rmpcd.on("song_change", function(old_song, song)
                log.info("Browser auth done, scrobbling")

                -- Only update now playing if we still have a song
                if song ~= nil then
                    lastfm_update_now_playing(
                        args.api_key,
                        args.shared_secret,
                        sk,
                        song.artist,
                        song.title,
                        song.album,
                        song.duration
                    )
                end
                local current_time = os.time()

                if song_start ~= nil and current_time - song_start > 30 then
                    scrobble(sk, old_song, song_start)
                end

                song_start = current_time
                current_song = song
            end)

            rmpcd.on("state_change", function(old_state, new_state)
                if new_state == "stop" and old_state == "play" then
                    local current_time = os.time()

                    if song_start ~= nil and current_time - song_start > 30 then
                        scrobble(sk, current_song, song_start)
                    end

                    song_start = nil
                    current_song = nil
                end
            end)
        end

        session_key = get_session_key(args.api_key, args.shared_secret, token)
        if session_key == nil then
            sync.set_interval(5000, function(handle)
                session_key = get_session_key(args.api_key, args.shared_secret, token)
                if session_key ~= nil then
                    register(session_key)
                    handle.cancel()
                end
            end)
        end
    end,
}
