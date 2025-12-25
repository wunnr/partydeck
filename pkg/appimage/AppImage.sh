#!/bin/sh

# You might need to restart your pc if sharun doesn't create `AppDir` in this directory (It should create dirs on its own)

# Grab release from https://github.com/wunnr/partydeck/releases/tag/v0.8.5 and extract it to the same dir as this .sh file
set -eu

ARCH="$(uname -m)"
SHARUN="https://raw.githubusercontent.com/pkgforge-dev/Anylinux-AppImages/refs/heads/main/useful-tools/quick-sharun.sh"

export ADD_HOOKS="self-updater.bg.hook"
#export UPINFO="gh-releases-zsync|${GITHUB_REPOSITORY%/*}|${GITHUB_REPOSITORY#*/}|latest|*$ARCH.AppImage.zsync"
export OUTNAME=partydeck-anylinux-"$ARCH".AppImage
export DESKTOP=partydeck.desktop
export ICON=./partydeck.png
export DEPLOY_OPENGL=0
export DEPLOY_VULKAN=0
export DEPLOY_DOTNET=0

#Remove leftovers
rm -rf AppDir dist appinfo

# ADD LIBRARIES
wget --retry-connrefused --tries=30 "$SHARUN" -O ./quick-sharun
chmod +x ./quick-sharun

# Point to binaries
./quick-sharun ./partydeck ./bin/gamescope-kbm ./bin/umu-run /usr/bin/fuse-overlayfs /usr/bin/bwrap

# Copy rest
mkdir -p ./AppDir/bin/bin
cp ./bin/gamescope-kbm ./Appdir/bin/bin

mkdir -p ./AppDir/bin/res
cp res/splitscreen_kwin.js ./AppDir/bin/res
cp res/splitscreen_kwin_vertical.js ./AppDir/bin/res

cp -r res/goldberg/ ./AppDir/lib

# Make AppImage
./quick-sharun --make-appimage

mkdir -p ./dist
mv -v ./*.AppImage* ./dist

echo "All Done!"
