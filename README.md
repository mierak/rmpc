# Rusty Music Player Client

Rmpc is a beautiful, modern and configurable terminal based Music Player Daemon client. It is
heavily inspired by [ncmpcpp](https://github.com/ncmpcpp/ncmpcpp) and
[ranger](https://github.com/ranger/ranger)/[lf](https://github.com/gokcehan/lf) file managers.

![preview](docs/public/preview.png)

## Get started

Description, configuration and installation methods can be found on [the rmpc website](https://mierak.github.io/rmpc/)

## Main Features

- Album cover art display if your terminal supports either of Kitty, Sixel, Iterm2 protocols, or via ueberzuggpp
- Cava integration for music visualisation
- Support for [synchronized lyrics](https://en.wikipedia.org/wiki/LRC_(file_format))
- Ability to play music from YouTube
- Configurable (T)UI
  - Configure what information(if any!) is displayed in the header
  - Configure what columns are displayed on the queue screen
  - Create any color theme you want
  - Every keybind can be changed, vim-like by default
- Ranger/LF-like three-column browser through your music library
- Basic playlist management
- Support scripting through basic CLI mode and script hooks

And more to come

> [!IMPORTANT]
> Rmpc is still in early development, and is not yet complete. It should be stable enough and I have
> been daily driving it for quite a while, but expect some bugs and possibly breaking changes to the
> config file.
