function rmpcd.notify(new_song)
	local artist
	if new_song.artist and type(new_song.artist) == "table" then
		artist = new_song.artist[1]
	elseif new_song.artist and type(new_song.artist) == "string" then
		artist = new_song.artist
	else
		artist = "Unknown Artist"
	end

	local title
	if new_song.title and type(new_song.title) == "table" then
		title = new_song.title[1]
	elseif new_song.title and type(new_song.title) == "string" then
		title = new_song.title
	else
		title = "Unknown Title"
	end

	process.spawn({ "notify-send", "Now playing: " .. artist .. " - " .. title })
end
