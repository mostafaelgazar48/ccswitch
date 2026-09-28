#!/bin/sh
# Double-click to install ccswitch into /usr/local/bin.
set -e

here=$(cd "$(dirname "$0")" && pwd)
dest=/usr/local/bin

echo "Installing ccswitch to $dest (macOS may ask for your password)..."
sudo mkdir -p "$dest"
sudo cp "$here/ccswitch" "$dest/ccswitch"
sudo chmod 755 "$dest/ccswitch"
sudo xattr -d com.apple.quarantine "$dest/ccswitch" 2>/dev/null || true

echo
echo "Installed: $("$dest/ccswitch" --version)"
echo "Open a new terminal and run: ccswitch --help"
echo "You can close this window."
