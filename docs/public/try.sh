#!/bin/sh

case "$(uname -m)" in
"x86_64" | "aarch64")
    arch=$(uname -m)
    ;;
*)
    echo "Your system is not supported. Supported systems: Linux"
    exit 1
    ;;
esac

case $(uname -s) in
"Linux")
    system="unknown-linux-gnu"
    ;;
*)
    echo "Your system is not supported. Supported systems: Linux"
    exit 1
    ;;
esac

mkdir -p /tmp/rmpc
tag=$(curl -s "https://api.github.com/repos/mierak/rmpc/releases/latest" | sed -n 's/\s*"tag_name": "\(.*\)",/\1/p')
curl -s -L --no-progress-meter "https://github.com/mierak/rmpc/releases/latest/download/rmpc-$tag-$arch-$system.tar.gz" | tar -C "/tmp/rmpc" -xz

printf "Thank you for trying out rmpc! Just one more thing.\n" >&2
printf "Rmpc needs to know the IP address and port to connect to.\n" >&2
printf "These values can be found in your mpd config. Please provide them below.\n\n" >&2

printf "Enter MPD's IP address (defaults to 127.0.0.1): " >&2
read -r mpd_ip
printf "Enter MPD's port (defaults to 6600): " >&2
read -r mpd_port

exec /tmp/rmpc/rmpc --address "${mpd_ip:-127.0.0.1}:${mpd_port:-6600}"
