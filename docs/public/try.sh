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
curl -s -L --no-progress-meter "https://github.com/mierak/rmpc/releases/latest/download/rmpc-$arch-$system.tar.gz" | tar -C "/tmp/rmpc" -xz

printf "Thank you for trying out rmpc! Just one more thing.\n" >&2
printf "Rmpc needs to know the IP address and port to connect to.\n" >&2
printf "These values can be found in your mpd config. Please provide them below.\n\n" >&2

printf "Enter MPD's IP address: " >&2
read -r mpd_ip
printf "Enter MPD's port: " >&2
read -r mpd_port

exec /tmp/rmpc/rmpc --address "$mpd_ip:$mpd_port"
