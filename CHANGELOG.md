# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- JumpToCurrent Queue action to make the cursor jump to the currently playing song
- Mouse support for modal popups
- List available decoder plugins from MPD via `ShowDecoders` action or `rmpc decoders`
- Ability to add and instantly play song under cursor. Bound to `Confirm` action
- A new `Lyrics` pane used to display synchronized lyrics.
- Missing default keybind for the Album Artists tab

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

### Fixed

- Songs not being sorted below directories in the Directories pane
- Scrolloff issues in Playlists pane after rename/move
- Few typos in UI and internal messages
- Click to select and rendering issues in SongInfo and Decoder modals
- Read stream not being emptied after encountering error while reading MPD's response
- Rows not wrapping in the keybinds modal when the screen is too small
- Unchecked panic inside the volume widget when volume exceeds certain value
- Several things that should have happened on song change were happening on every `Player` event, ie. seeking

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

[unreleased]: https://github.com/mierak/rmpc/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/mierak/rmpc/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/mierak/rmpc/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/mierak/rmpc/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/mierak/rmpc/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/mierak/rmpc/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/mierak/rmpc/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/mierak/rmpc/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/mierak/rmpc/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mierak/rmpc/releases/tag/v0.1.0
