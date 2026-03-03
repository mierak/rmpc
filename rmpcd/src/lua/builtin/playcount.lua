function rmpcd.playcount(new_song)
	local sticker, err = mpd.get_sticker(new_song.file, "playcount")
	if err then
		log.info("Error retrieving playcount sticker for '%s': %s", new_song.file, err)
		return
	end

	if sticker == nil then
		log.info("No playcount sticker found for '%s'. Initializing to 1.", new_song.file)
		mpd.set_sticker(new_song.file, "playcount", "1")
	else
		log.info("Playcount sticker found for '%s'. Incrementing playcount.", new_song.file)
		local count = tonumber(sticker) or 0
		mpd.set_sticker(new_song.file, "playcount", tostring(count + 1))
	end
end
