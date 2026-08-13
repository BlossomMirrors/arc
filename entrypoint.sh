#!/bin/sh
set -e

echo "Refreshing Flatpak appstream data..."
if ! flatpak update --appstream --system; then
    echo "warning: appstream refresh failed, starting with the data baked into the image" >&2
fi

exec /usr/local/bin/arc-daemon
