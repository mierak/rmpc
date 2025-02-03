# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Support for repeating lyrics in lrc
- `vertical_align` and `horizontal_align` to album art config, supports kitty, sixel and iterm2
- Support for fixed Pane size in Tabs
- Support for displaying MPD stickers in the header and queue table
- Support for manipulation of MPD stickers via CLI
- Support for globs/multiple files and songs outside music database (with socket connection) in the `add` cli command
- Added new `layout` config option which allows to move around the base components
- `PageUp` and `PageDown` actions
- Configurable timeout for connection to MPD
- `$PID`, `$VERSION` to external commands
- `$HAS_LRC`, `$LRC_PATH` to `on_song_change`
- `remote` command to cli which allows for IPC with running rmpc instances
- Example script to automatically download lyrics from [https://lrclib.net/](https://lrclib.net/)
- Added `max_fps` to config
- Introduced `StateV2` property as a replacement for `State`. It has additional config properties compared to its predecesor.
- Introduced `RandomV2`, `ConsumeV2`, `RepeatV2` and `SingleV2` properties as a replacement for their respective earlier versions.
They have additional config properties compared to their predecesors.

### Changed

- Increased default album art `max_size_px` to `(1200, 1200)`.
- Improved navigation between Pane splits by including recency bias
- CLI now parses only the required part of the config
- Status messages will now disappar automatically even when idle
- Lyrics should now sync better because they are now scheduled precisely instead of periodically
- MSRV to 1.82

### Fixed

- `ToggleConsume` and `ToggleSingle` causing playback to stop
- Styling not being applied to Bitrate and Crossfade props
- Refactored and greatly simplified image backends
- Potential infinite loop in lyrics indexing
- `lsinfo` parsing playlist entries incorrectly
- Missing border in tabs with `border_type: Single`
- Properly escape strings in mpd protocol
- Preview for songs outside of the music database not working in playlists
- AddToPlaylist not working for local songs
- rmpc waiting potentionally forever for MPD's response

### Deprecated

- `theme.tab_bar.enabled` replaced by layout configuration
- `State` header property
- `RandomV2`, `ConsumeV2`, `RepeatV2` and `SingleV2` header properties

## [0.7.0] - 2024-12-24

### Added

- JumpToCurrent Queue action to make the cursor jump to the currently playing song
- Mouse support for modal popups
- List available decoder plugins from MPD via `ShowDecoders` action or `rmpc decoders`
- Ability to add and instantly play song under cursor. Bound to `Confirm` action
- Theme: add `symbols.ellipsis` to customize the ellipsis when text need to be truncated
- A new `Lyrics` pane used to display synchronized lyrics.
- Missing default keybind for the Album Artists tab
- Allow stop action to work in paused state
- Select functionality to the queue, selected songs can be moved up and down in the queue at the same time using the MoveUp/Down actions
- Selected songs in queue can now be removed all at once from the Queue with the Delete action
- InvertSelection action
- Show album date in the `Artists` and `AlbumArtists` panes
- Config options to sort albums by date or name and to hide or show album date in in the `Artists` and `AlbumArtists` panes
- Rmpc will now try to reconnect and reinitialize on losing connection to mpd

### Changed

- Queue table now remembers cursor position when you switch tabs
- Browser panes now remember cursor position in the root level when you switch tabs
- Refactor and split utils module
- Set binary limit to 5MB
- Disabled album arts for songs over http(s). Can be brought back by changing `album_art.disabled_protocols`
- Improves the usability and clarity of the queue deletion confirmation modal
- `width_percent` config option in `song_table_format`. Replaced by `width`.
- Deletion of a playlist now requires user confirmation
- Default keybinds for tabs to make space for the Album Artists tab
- Swapped default single and consume keybinds
- Clear album art and song in the header when the playback stops
- Refactored confirm modal into a generic one
- Refactored rename playlist and save queue modal into a generic modal with single input
- Refactored add to playlist modal into generic select modal
- Refactored MPD client out of a UI thread. Rmpc now also requires only single connection to MPD.

### Fixed

- Songs not being sorted below directories in the Directories pane
- Scrolloff issues in Playlists pane after rename/move
- Few typos in UI and internal messages
- Click to select and rendering issues in SongInfo and Decoder modals
- Read stream not being emptied after encountering error while reading MPD's response
- Rows not wrapping in the keybinds modal when the screen is too small
- Unchecked panic inside the volume widget when volume exceeds certain value
- Several things that should have happened on song change were happening on every `Player` event, ie. seeking
- Improved handling of errors while reading MPD's response
- Adjust scrollbar position in browser panes when `track` symbol is empty
- Scrolloff not applying on the very first render

### Deprecated

- `width_percent` config option in `song_table_format`. It will continue to work for now, but will be removed in the future.

## [0.6.0] - 2024-10-28

### Added

- Arrow keys as secondary navigation keybinds alongside hjkl
- Support for basic control with mouse. Check docs for more info.
- Scrolloff option to keep some context the various lists/tables
- Update/rescan CLI commands to refresh MPD's database
- Support MPD password via config, env vars and CLI
- ShowInfo action to queue pane. Displays metadata of the song under cursor in a modal popup.
- ShowCurrentSongInfo global action. Displays metadata of the song currently playing song in a modal popup.

### Changed

- Removed left/right arrows as default keybinds for next/previous tab. You can still put these back by editing your config.
- Filtering is now incremental
- Up/Down actions do not wrap around anymore. You can get the previous behavior back with the `wrap_navigation` config option
- Allow seeking while paused

### Fixed

- Rmpc now logs warnings and errors in CLI mode to stderr
- try to clean up after yt-dlp in case it fails
- Album art not clearing properly after direct tab switch
- Events being duplicated when panes were present in multiple tabs
- Ueberzugpp redrawing album art while in an inactive TMUX window/session
- Fix improper scrollbar rendering with some symbols being empty
- Removed duplicated tags in metadata view of a song

## [0.5.0] - 2024-09-27

### Added

- Added ability to bind external scripts, they are executed with info about MPD and rmpc in environment variables
- Added `--path` filter to `song` command
- Added ability to configure the search screen
- Added this changelog
- Added `tabs` config, which lets you customize what tabs you want to use and even mix and match them.
- Rmpc now respects `MPD_HOST` and `MPD_PORT` environment variables.
- Display current_match_idx/total_matches in the browser screens when using a filter

### Changed

- Allow `-1` as a valid volume value in response to status command for improved backwards compatibility
- Improved logging of MPD command parsing failures
- Refactored how image protocol backends request render by moving channels to context
- Make some things more robust by checking commands supported by MPD server (albumart/readpicture/getvol)
- Check MPD protocol version for single command
- `version` and `debuginfo` commands now always display `CARGO_PKG_VERSION`

### Deprecated

- `QueueTab`, `DirectoriesTab`, `ArtistsTab`, `AlbumsTab`, `PlaylistsTab` and `SearchTab` actions are now deprecated.
  They will continue to work with the default config, but you should migrate to `FocusTab(<tabname>)`

### Removed

- `album_art_position` and `album_art_width_percent` from theme config. They have been replaced by `tabs` config.
  All their functionality can still be achieved by using the new `tabs`.

### Fixed

- Do not query album art if it is disabled
- Panic with zero-width browser column
- Browsers now keep their filter when pushed down on the stack

## [0.4.0] - 2024-08-26

### Added

- Added groups to property formatters
- Added support for sixel image protocol
- Added `AddAll` keybind
- Added ability to execute a script on song change with info about current song

### Fixed

- Fixed filename property behavior in proprty formatters
- Added missing text color to default theme

### Removed

- Commit date to help nix pkg

## [0.3.0] - 2024-08-12

### Added

- Added support for iterm2 inline image protocol
- Added support for ueberzugpp album art backend
- Added basic manpage and cli completions
- Made song format configurable in browsers screens
- Implemented basic runtime dependency checking and debuginfo command
- Added option to follow current song in the queue table
- Added AUR and nix to install methods
- Added aarch64 and musl targets

### Fixed

- Compilation issues for tests in release mode
- Modals over album art not clearing properly
- Fixed TMUX passthrough testing

## [0.2.1] - 2024-07-27

### Added

- Handling of terminal resize events

### Fixed

- Fixed yt-dlp download format

## [0.2.0] - 2024-07-26

### Added

- Added keybinds help modal
- Implement command mode/cli
- Added outputs config modal/cli
- Added get volume, status info, song info commands
- Added inital youtube playback support
- Introduced worker queue

### Fixed

- Fixed warning message when kitty image protocol is not supported

### Changed

- Made image compression/serialization asynchronous

## [0.1.2] - 2024-07-01

## [0.1.1] - 2024-06-22

## [0.1.0] - 2024-06-21

[unreleased]: https://github.com/mierak/rmpc/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/mierak/rmpc/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/mierak/rmpc/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/mierak/rmpc/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/mierak/rmpc/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/mierak/rmpc/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/mierak/rmpc/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/mierak/rmpc/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/mierak/rmpc/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/mierak/rmpc/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mierak/rmpc/releases/tag/v0.1.0
