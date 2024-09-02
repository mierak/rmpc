# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

* Added ability to bind external scripts, they are executed with info about MPD and rmpc in environment variables
* Added `--path` filter to `song` command
* Added ability to configure the search screen
* Added this changelog

### Changed

* Allow `-1` as a valid volume value in response to status command for improved backwards compatibility
* Improved logging of MPD command parsing failures
* Refactored how image protocol backends request render by moving channels to context
* Make some things more robust by checking commands supported by MPD server (albumart/readpicture/getvol)
* Check MPD protocol version for single command

### Deprecated

### Removed

### Fixed

* Do not query album art if it is disabled


## [0.4.0] - 2024-08-26

### Added
* Added groups to property formatters
* Added support for sixel image protocol
* Added `AddAll` keybind
* Added ability to execute a script on song change with info about current song

### Fixed

* Fixed filename property behavior in proprty formatters
* Added missing text color to default theme

### Removed

* Commit date to help nix pkg

## [0.3.0] - 2024-08-12


### Added

* Added support for iterm2 inline image protocol
* Added support for ueberzugpp album art backend
* Added basic manpage and cli completions
* Made song format configurable in browsers screens
* Implemented basic runtime dependency checking and debuginfo command
* Added option to follow current song in the queue table
* Added AUR and nix to install methods
* Added aarch64 and musl targets

### Fixed

* Compilation issues for tests in release mode
* Modals over album art not clearing properly
* Fixed TMUX passthrough testing


## [0.2.1] - 2024-07-27

### Added

* Handling of terminal resize events

### Fixed

* Fixed yt-dlp download format


## [0.2.0] - 2024-07-26

### Added

* Added keybinds help modal
* Implement command mode/cli
* Added outputs config modal/cli
* Added get volume, status info, song info commands
* Added inital youtube playback support
* Introduced worker queue

### Fixed

* Fixed warning message when kitty image protocol is not supported

### Changed

* Made image compression/serialization asynchronous

## [0.1.2] - 2024-07-01

## [0.1.1] - 2024-06-22

## [0.1.0] - 2024-06-21

[unreleased]: https://github.com/mierak/rmpc/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/mierak/rmpc/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/mierak/rmpc/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/mierak/rmpc/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/mierak/rmpc/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/mierak/rmpc/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/mierak/rmpc/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mierak/rmpc/releases/tag/v0.1.0
