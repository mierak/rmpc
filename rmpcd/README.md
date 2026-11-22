# rmpcd

> [!WARNING]
> This project is in its very early stages and is not ready for daily use. Expect
> changes to basically everything with no regards to backwards compatibility at
> this point.

A companion MPD client to rmpc intended to run as a daemon.

## Get started

Description, configuration and installation methods can be found on the [rmpcd website](https://rmpc.mierak.dev/rmpcd/)

## Features

- Fully configurable with Lua
- MPRIS server implementation
- Idle inhibition when music is playing
- Full plugin support
- Built-in plugin manager
- A few official plugins:
  - [Last.fm](https://github.com/rmpc-org/rmpcd-lastfm)
  - [Playcount](https://github.com/rmpc-org/rmpcd-playcount)
  - [Notify](https://github.com/rmpc-org/rmpcd-notify)
  - [Lyrics](https://github.com/rmpc-org/rmpcd-lyrics)
  - [Discord rich presence](https://github.com/rmpc-org/rmpcd-discord-rich-presence)
  - [Last played](https://github.com/rmpc-org/rmpcd-lastplayed)

- A growing list of [community created plugins](https://github.com/rmpc-org/rmpcd-plugins)

## Installation

Rmpcd can currently only be built from source code, installed via AUR or using the provided Nix flake.

### AUR

```bash
yay -S rmpcd-git
```

### Building from source

This assumes you have git and rust installed already.

```bash
# Either with cargo directly
cargo install --git https://github.com/mierak/rmpc.git rmpcd

# Or clone the repo and run
git clone https://github.com/mierak/rmpc.git
cd rmpc

cargo run --release --package rmpcd -- init
cargo run --release --package rmpcd
```

### NixOS

With the flake added to your flake inputs, add the package to your system configuration:

```nix title="configuration.nix"
{ inputs, pkgs, ... }:

{
  environment.systemPackages = with pkgs; [
    inputs.rmpc.packages.${pkgs.stdenv.hostPlatform.system}.rmpcd
  ];
}
```

## Setup

After installation run:

```bash
rmpcd init
# You will see output similar to:
2026-03-28T23:42:17.918458Z  INFO rmpcd/src/main.rs:109: Created default config at '/home/<user>/.config/rmpcd/init.lua'
2026-03-28T23:42:17.918924Z  INFO rmpcd/src/main.rs:124: Created Lua API type definitions at '/home/<user>/.local/share/rmpcd/lua'
```

This will create a default `init.lua` at the path shown in the log. It will also generate a
`.luarc.json` file pointing to the type definitions, allowing your editor to provide autocompletion
if you have the [Lua language server](https://github.com/LuaLS/lua-language-server) installed and
configured.

Installing plugins will also download their type definitions and they should work automatically.

Rmpcd will keep the generated type definitions up to date automatically. Make sure to not edit these
files directly.
