# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- `extra_yt_dlp_args` to pass in more things to yt-dlp if required
- **Breaking** `ExternalCommand` can now have arguments supplied at runtime. This will break your existing
keybinds if they contained either `{` or `}`. You will now need to escape these by doubling them up: `{{` and `}}`.
- `scroll_speed` to `song_table_format`
- `album_art.custom_loader` to allow for more flexibility when choosing the album art image
- added `CopyToClipboard()` action
- `stop_on_exit` config option to automatically stop playback when exiting rmpc

### Changed

- Improved error message when invalid `default_album_art_path` path is provided
- Environment variables are now resolved when parsing paths in the config file or theme

### Fixed

- Theme hot reload not working when it was set using the `theme.ron` form
- Issue with Iterm2 image protocol not rendering images sometimes
- Moved `Update` and `Rescan` back to `u` and `U` respectively because they were conflicting with other
keybinds
- Fixed a benign error when deleting items from the queue really fast


## [0.11.0] - 2026-02-01

### Added

- `AfterCurrentAlbum` and `BeforeCurrentAlbum` to `AddOptions` keybind
- `order` option to `album_art`, sets whether to check embedded image or cover image file first
- Added hot reload for lyrics and corresponding `enable_lyrics_hot_reload` config option
- `enable_lyrics_index` to disable `lyrics_dir` indexing on startup
- Support abstract socket in `MPD_HOST` and config file on Linux
- Improved text input fields, supports shortcuts similar to default bash, like ctrl+w to delete a
word, ctrl+f/b to move forward/backwards etc.
- `clear` options to keybinds, keep the default keybinds and only override the specified keybinds
if false
- Triggering `JumpToCurrent` twice will now center the currently playing song
- Keybinds now support sequences of keys instead of a single key chord. Existing keybinds should work
the same.
- Added `SortByColumn(_)` to sort the queue by the given column. Clicking the table header also sorts
the queue by the given column.
- Added `load` and `save` commands to the CLI
- Added more config options to pane definitions, you can now define border titles, their style, position
as well as fully custom border symbol sets
- Added `Bits()`, `SampleRate()` and `Channels()` properties to song and status
- Downloads from ytdlp not show inside a modal letting you have some basic control over them as well
- Added `auto_open_downloads` to config to control whether the download modal should open automatically
after starting a ytdlp download
- Added `scroll_amount` config option
- Added `Added()` and `LastModified()` song properties
- Added `NotExact` and `NotRegex` search options
- Added `--clean` aragument
- Added `background_color` to pane configs

### Changed

- **Breaking** Keybinds now only override the defaults. Meaning your configured keybinds are combined
with the default ones. Set `clear` to true to keep the old behavior.
- **Breaking** `show_song_table_header` has been removed. The queue header is now in a separate `QueueHeader`
pane. You will need to update your config to include it in your tabs.
- **Breaking** `draw_borders` has been deprecated. The `Tabs` pane is no longer affected by this, use the
borders configuration on the pane itself instead. This now only affects borders in the browser panes
and this will be romeved in the future as well.
- **Breaking** `current_item_style` and `highlighted_item_style` now merge on top of the item's style
instead of having defaults, specify all the style properties (fg, bg, modifiers) to keep the old look
-- **Breaking** progress bar's `use_track_when_empty` now defaults to true
- `duration_format` is now a string template and configurable for song durations. Use `"%m:%S"` for
classic format, `"%M:%S"` for zero-padded minutes, or custom templates with tokens like `%d/%D` (days),
`%h/%H` (hours), `%m/%M` (minutes), `%s/%S` (seconds), `%t` (total seconds)
- `Queue` pane no longer has empty space on the sides
- Moved docs to a new [repository](https://github.com/rmpc-org/rmpc-org.github.io) and [domain](https://rmpc.mierak.dev/)
- Rmpc now checks for embedded image first and cover image in a file second by default, this can be
configured with the new `album_art.order` option
- The queue table should now be more performant for a very large number of items
- Default keybinds have been updated. This will not affect you if have a properly setup config file.
- Default theme has been updated. This will not affect you if have a properly setup config file.
- Improved how config and theme files are read. Config file will now properly try all available paths
instead of just the first one. When theme file is not found it will now correctly present an error
instead of silently falling back to the default theme.
- Improved rendering of the `Block` backend, it should now be less noisy
- Synchronized lyrics now prefer showing more future lyrics rather than more past lyrics

### Fixed

- Ignore rare phantom inputs from querying terminal for protocol support on startup
- Fix directories pane not fetching data after using the `Confirm` action to enter a directory
- Album art (sixel and iterm2) sometimes being aligned to an incorrect pane when in tmux splits
- Album art (sixel and iterm2) not rendering after detaching and reattaching from tmux session
- `highlighted_item_style` having unstyled spaces between columns in the queue table
- rmpc not correctly removing keyboard enhancement flags
- Unfocusable panes being unable to receive mouse events when put inside a tab
- Tabs now respect changes in `components` when they change via remote ipc
- Album art will now be completely disabled when zellij is detected and image method is set to `Auto`.
You can still force other image backend via config.
- yt-dlp integration will now try to issue a database update if the downloaded file cannot be found,
should fix cases with `cache_dir` being set inside MPD's music directory
- Added missing confirmation when deleting playlist/songs from playlist
- Some very minor speedups in queue with very large queue sizes
- `ScanStatus` not working
- A harmless error about stickers not being supported will no longer show up when previewing a song
in a browser pane for the first time will no longer show up
- Ellipsis in Queue now properly works on the fully constructed property instead of on its parts
- Fixed the `Block` image backend being cut off a bit
- ueberzug image backend failing on first start

### Removed

- `Save` and `AddToPlaylist` queue actions. These have been undocumented and mostly unused. Use
the `Save()` navigation action instead.

## [0.10.0] - 2025-11-11

### Added

- Added --interactive: interactive picker (TUI list / CLI prompt) with --limit N for `searchyt`.
- Added youtube song by name support: `searchyt "query"` (uses first YouTube result; TUI supported).
- Added interactive scrollbar support to browser panes and search pane results:
- Added `rmpc remote keybind` command to emulate key presses in running instances
- Added `rmpc remote switch-tab` command to switch tabs directly without relying on keybinds
- Added new `Volume` pane with mouse control support.
- Added `Transform` properties and `Truncate` transformation
- Added `sendmessage` cli command for inter client communication
- Support for youtube playlists
- Added configurable kebyind for adding items to the Queue - `AddOptions`
- Multiple marked items can now be moved inside playlists
- New `song_table_album_separator` theme property
- Add `queue` cli command to print contents of the current queue
- New `Partition` keybind to facilitate partition switching and management
- New `Partition` status property to show rmpc's current partition
- `PREV_SONG` and `PREV_ELAPSED` env variables in `on_song_change`
- New `ContextMenu()` action
- Support Kitty's keyboard protocol
- Remote query command
- Config option for global lyrics offset - `lyrics_offset_ms`
- add `exec_on_song_change_at_start` config option
- Added `album_date_tags` config option to specify priority order of metadata tags for album dates
- Added an ability to specify relative/absolute theme path in config file
- Added `keep_state_on_song_change` and equivalent flag to cli
- Added `ignore_leading_the` when sorting entries in browsers
- More information about the system to debuginfo
- Stickers are now usable in `browser_song_format`
- Rating support, rating can be set on a song. The rating can be displayed in the queue table and
browser panes as well as searched for in the Search pane
- Liked state support, songs can now be liked, disliked or set to neutral and searched for in the
Search pane
- Added `Replace` transform
- Added section filter to docs search
- `search_button` config option for search pane
- Configurable `Save()` keybind in the navigation section
- Added `SeekToStart` global action to seek to the beginning of the currently playing track
- `DeleteFromPlaylist` action to delete songs in browsers panes from selected playlist
- Added `CrossfadeUp` and `CrossfadeDown` global actions
- `listall` command to cli
- `use_track_when_empty` to progress bar, renders track symbol instead of start/end when they are empty

### Changed

- **Breaking**: TabName equality comparison is now case-insensitive.
- **Breaking**: `$SELECTED_SONGS` in queue now contains marked songs as well
- Add support for soundcloud on `searchyt`
- Improved `.lrc` lyrics files parser performance and fixed parsing issues
- Normalized duration formatting across `QueueTimeTotal` and `QueueTimeRemaining` properties for both standard (MM:SS/H:MM:SS) and verbose formats
- Confirm action in browsers, which either opens a directory or adds the hovered song to the queue
no longer exhibits the latter behavior. It now instead replaces the queue with all songs in the
directory.
- Removed `Add`, `Insert`, `AddAndReplace`, `AddAll`, `InsertAll` and `AddAllReplace` from the code.
They are now mapped to the new `AddOptions` action. Existing configs are not affected and will
continue to work.
- Scrolling behavior to be more natural - scrolling now actually scrolls the area instead of simply
going to the next item
- Scrollbars now represent the viewport position instead of the currently selected item position
- Remote commands now check for `$PID` env variable, meaning `--pid` argument is no longer needed for
remote commands inside scripts triggered by rmpc
- `AddToPlaylist` binding handles marked songs rather than only the one under your cursor.
- Paused playback state is now kept by default when using the `NextTrack/PreviousTrack` keybinds. Use
`keep_state_on_song_change` to disable this
- Default theme now includes lyrics pane above the album art on queue tab
- Browsers now properly use case insensitive sorting
- Refactored and improved image backend detection
- `JumpToCurrent` now jumps to last playing song in stopped state
- Directories now keep their state when going back out of them
- Nord theme update

### Fixed

- Config deserialization errors are now printed to stderr during startup, making them visible even when subsequent initialization steps fail
- Fixed `QueueTimeRemaining` not updating remaining time
- `ModalClosed` event now correctly gets dispatched only after all modals were closed
- Preview no longer disappears in search when returning to the search form while the results are
scrolled down
- Adding entries without album adding not intended songs when using split by date
- Volume parsing if MPD's volume was set to higher value than 255 via external means
- Fix improper handling of remote theme change
- Tilde not being expanded for yt-dlp on non-linux platforms
- Remove mention of `tab_bar.enabled` from docs
- Konsole terminal now does not autodetect to Kitty image protocol, it instead uses ueberzugpp if
available and Block if not
- Fix Iterm2 image protocol sometimes rendering too late
- Fix playlists not using playlist style and icon in the Playlists pane
- Order of added songs when adding them from browser panes
- Marked items in Queue not being cleared on database update
- Improved behavior if rmpc happens to panic at certain time
- Group not falling back to its default value
- Panic when only one tab is defined in the config

### Deprecated

- `Save` keybind in the queue section. Use the new configurable `Save()` keybind in the navigation
section instead

## [0.9.0] - 2025-06-23

### Added

- `theme.modal_backdrop` adds a visual backdrop to modals
- rmpc will now reload config changes automatically
- remote command to change theme
- `on_resize` which is called whenever rmpc is resized
- `--theme` cli argument to override theme in the config file
- new `Browser` pane
- Add ability to scroll and cycle `Property` panes when they do not fit their area
- `browser_song_sort` which is a list of properties which defines how the songs are sorted in the browser panes
- `directories_sort` to configure how the directories pane is sorted
- `FileExtension` property
- Better error message inside a modal when reading of config fails
- Support for multiple entries in one tag. In formats they get separated by `format_tag_separator` and in metadata
  lists they are listed as multiple entries.
- Add new widget `ScanStatus` that indicates if the MPD database is being updated.
- Add new global keybinds for Update and Rescan actions.
- Addrandom CLI command and `AddRandom` action which displays a modal allowing you to add random songs to the queue
- Introduced `preview_label_style` and `preview_metadata_group_style` in theme config
- Added support for soundcloud and nicovideo to `addyt`
- Added `AddReplace` and `AddAllReplace` actions which work smimilarly to `Add` and `AddAll` but replace the current queue instead of appending
- Added `Insert` and `InsertAll` actions which work similarly to `Add` and `AddAll` but insert after the playing song
- Added `Shuffle` queue action allowing you to shuffle the whole queue or selected range(s)
- Added `QueueLength` status property which displays number of songs in the current queue
- Added `QueueTimeTotal` and `QueueTimeRemaining` status properties which display sum of time of songs in your queue and of the remaining songs respectively
- Add Nord community theme
- Add `center_current_song_on_change` to center song in the queue when it changes
- Added `togglerandom`, `togglesingle`, `togglerepeat` and `toggleconsume` CLI commands
- Added `ToggleSingleOnOff` and `ToggleConsumeOnOff` global actions which skip oneshot for their respective mode
- Added `ActiveTab` status property showing the name of currently active tab
- Added `--rewind-to-start` CLI argument for the previous action, allowing users to rewind to the start of the currently playing song when
  navigating to the previous track.
- Sort keybinds in the help modal
- Include song metadata in `ExternalCommand` env
- Added `Block` AlbumArt Support
- Add `level_styles` config option for various status message levels
- Add filtering to the keybinds modal
- Added info modal to the playlist
- Added initial partition support which allows you to connect to partition specified as a CLI argument
- Added `listpartitions` CLI
- Added Start and End Boundaries to ProgressBar increasing its Customizability
- Added components to the theme. Components are user-defined reusable parts of TUI.
- Added `rewind_to_start_sec` config option. If elapsed time is past the configured value, the song will be rewound to start instead.
- Added `reflect_changes_to_playlist` config option. This makes changes to the queue reflect to the stored playlist if any.
- Added `multiple_tag_resolution_strategy` to choose which tag value to display when multiple values are present
- Added `maps_three_symbols` test for progress bar. This will help avoid any errors while changing progress bar code in future
- Added `PopConfigErrorModal` so theat the config error modals are automatically removed when the config reloads and is found correct
- Added style configuration for dir and song symbols in browsers
- Added `lyrics` config option `timestamp` for showing line timestamp
- Added new `Cava` pane
- Added `mpd_idle_read_timeout_ms`
- Added FAQ section to the docs
- Directories pane now displays playlists located in your music directory. Also added `show_playlists_in_browser`
  to hide them.
- Added ratio size. This size is relative to its parent size.
- Added `plugin` field to Outputs modal and command
- Fixed order when adding multiple items with `Insert` and `InsertAll`
- Added `Position` property which shows song's position in the Queue
- `Close` action now clears marked items

### Changed

- **Breaking**: Songs are no longer sorted by their `browser_song_format`. The new `browser_song_sort` is used instead
- **Breaking**: Some tags can now be arrays of values instead of a single value if multiple values are in the given id3 tag when listing song metadata via cli.
- **Breaking**: For CLI which return song info: `last-modified` and `added` are no longer in songs' metadata, they are at the top level object instead now
- **Breaking**: `ShowInfo` queue action has been moved to `navigation`. It is now more general and works in playlists as well.
- The first lyrics will now only be highlighted once reached
- `Filename` property no longer includes file extension, use `FileExtension` if you want to keep it
- Migrate to Rust 2024 and raise MSRV to 1.85
- refactor `DirOrSong` to a separate file
- Lyrics will be wrapped if it is longer than the pane width
- Refactored yt-dlp to make it easier to add support for more hosts
- `scrollbar` theme option now also accepts `None` as a valid value to hide all scrollbars in rmpc
- `TogglePause` in both the keybind and CLI to issue play if the current state is stopped
- Made lyrics index matching more lenient
- Changed the logging path from `/tmp/rmpc.log` to `/tmp/rmpc_${UID}.log`
- Allow spaces between values in RGB colors and improved error messages

### Fixed

- Lyrics with fractions of seconds which weren't to 2s.f. being parsed incorrectly
- Lyrics with metadata fields containing ']' not being indexed
- Album art staying on the old one when in tmux and not visible
- Fixed catpuccin theme not being up to date in the docs
- Handle invalid utf8 characters
- Improve reconnection behavior of MPD client
- Improve performance of the queue table by not calculating rows that are not visible
- Tilde not being expanded in `default_album_art_path`
- Filter overlapping song table when `show_song_table_header` was set to false
- Remove extra space at the start of every lyrics line in case the lrc file had space between timestamp and content
- Styles for `Group` not working in the queue table
- Allow playlist with no metadata for preview. Happens in entries from http for example.

## [0.8.0] - 2025-02-16

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
- Introduced `StateV2` property as a replacement for `State`. It has additional config properties compared to its predecessor.
- Introduced `RandomV2`, `ConsumeV2`, `RepeatV2` and `SingleV2` properties as a replacement for their respective earlier versions.
  They have additional config properties compared to their predecessors.
- `borders` configuration in the tabs configuration
- A new `Property` pane

### Changed

- Increased default album art `max_size_px` to `(1200, 1200)`.
- Improved navigation between Pane splits by including recency bias
- CLI now parses only the required part of the config
- Status messages will now disappear automatically even when idle
- Lyrics should now sync better because they are now scheduled precisely instead of periodically
- MSRV to 1.82
- Song metadata is now split into groups

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
- rmpc waiting potentially forever for MPD's response
- Adding songs which do not belong to any album not working in `Artists` and `AlbumArtists` panes not working
- Songs metadata not being sorted in preview column
- Prevent album art rendering when modal is open
- Fix panic when `ProgressBar` pane had insufficient height
- Middle mouse click not working in search when browsing songs

### Deprecated

- **Breaking**: `border_type` in tabs config. It has been replaced by the new and more powerful `borders`
- **Breaking**: `theme.tab_bar.enabled`. It has been replaced by layout configuration.
- `State` header property
- `Random`, `Consume`, `Repeat` and `Single` header properties

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

- Fixed filename property behavior in property formatters
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
- Added initial youtube playback support
- Introduced worker queue

### Fixed

- Fixed warning message when kitty image protocol is not supported

### Changed

- Made image compression/serialization asynchronous

## [0.1.2] - 2024-07-01

## [0.1.1] - 2024-06-22

## [0.1.0] - 2024-06-21

[unreleased]: https://github.com/mierak/rmpc/compare/v0.11.0...HEAD
[0.10.0]: https://github.com/mierak/rmpc/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/mierak/rmpc/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/mierak/rmpc/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/mierak/rmpc/compare/v0.7.0...v0.8.0
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
