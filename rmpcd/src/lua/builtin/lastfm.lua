-- Very much a WIP and not really working
local API_KEY = ""
local SHARED_SECRET = ""
local auth_url = "http://ws.audioscrobbler.com/2.0/?method=auth.gettoken&api_key=" .. API_KEY .. "&format=json"

local is_authenticated = false
local is_authenticating = false
local token = "-qp1U07LoZzKFPeCgVzkNBSf9jj8Hk9R"
local session_key = "_nB4ZcZo21R_WGn9vC3SeIm-RmX_uZYG"

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

local function lastfm_update_now_playing(artist, track, album, duration_seconds)
    local params = {
        method = "track.updateNowPlaying",
        api_key = API_KEY,
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

    params.api_sig = lastfm_api_sig(params, SHARED_SECRET)

    local resp = http.post("https://ws.audioscrobbler.com/2.0/", {
        headers = {}, -- optional
        body = nil, -- nothing in body
        params = params, -- becomes querystring
    })

    if resp.code ~= 200 then
        log.error("Last.fm updateNowPlaying failed: HTTP " .. resp.code)
        util.dump_table(resp)
        return nil, resp
    end

    return resp:json(), resp
end

function auth()
    is_authenticating = true
    local response = http.get(auth_url)
    if response.code ~= 200 then
        log.error("Failed to get Last.fm auth token: HTTP " .. response.code)
        return
    end

    token = response:json().token
    util.dump_table(response)

    process.spawn({ "xdg-open", "http://www.last.fm/api/auth/?api_key=" .. API_KEY .. "&token=" .. token })
    is_authenticated = true
    is_authenticating = false
end

function rmpcd.lastfm(song)
    lastfm_update_now_playing(song.artist, song.title, song.album, song.duration)
    return

    -- if is_authenticating then
    --     log.info("Already authenticating with Last.fm, please complete the authentication in your browser")
    --     return
    -- end
    -- if not is_authenticated then
    --     auth()
    --     return
    -- end
    --
    -- local api_sig = "api_key" .. API_KEY .. "method" .. "auth.getSessiontoken" .. token .. SHARED_SECRET
    -- local api_sig_hash = util.md5(api_sig)
    --
    -- -- Fetch session token
    -- local session_response = http.get(
    --     "http://ws.audioscrobbler.com/2.0/?method=auth.getSession&api_key="
    --         .. API_KEY
    --         .. "&token="
    --         .. token
    --         .. "&api_sig="
    --         .. api_sig_hash
    --         .. "&format=json"
    -- )
    -- session_key = session_response:json().session.key
    --
    -- util.dump_table(session_response)
end
