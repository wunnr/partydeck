#!/bin/bash
export GBE_FORK_LINUX_RELEASE_URL=$(curl -s https://api.github.com/repos/Detanup01/gbe_fork/releases/latest | grep "browser_download_url.*emu-linux-release\.tar\.bz2" | cut -d '"' -f 4)
export GBE_FORK_WIN_RELEASE_URL=$(curl -s https://api.github.com/repos/Detanup01/gbe_fork/releases/latest | grep "browser_download_url.*emu-win-release\.7z" | cut -d '"' -f 4)
export UMU_LAUNCHER_RELEASE_URL=$(curl -s https://api.github.com/repos/Open-Wine-Components/umu-launcher/releases/latest | grep "browser_download_url.*umu-launcher-.*-zipapp\.tar" | cut -d '"' -f 4)

mkdir -p deps/releases

curl -L -o deps/releases/emu-linux-release.tar.bz2 "$GBE_FORK_LINUX_RELEASE_URL"
curl -L -o deps/releases/emu-win-release.7z "$GBE_FORK_WIN_RELEASE_URL"
curl -L -o deps/releases/umu-launcher-latest-zipapp.tar "$UMU_LAUNCHER_RELEASE_URL"

rm -rf deps/releases/gbe-linux-release deps/releases/gbe-win-release

tar -xf deps/releases/emu-linux-release.tar.bz2 -C deps/releases
mv deps/releases/release deps/releases/gbe-linux-release
7z x -aoa deps/releases/emu-win-release.7z -o"deps/releases"
mv deps/releases/release deps/releases/gbe-win-release
tar -xf deps/releases/umu-launcher-latest-zipapp.tar -C deps/releases