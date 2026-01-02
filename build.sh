#!/bin/bash

cargo build --release && \
rm -rf build && \
mkdir -p build/ build/res build/bin && \
mkdir -p build/res/goldberg/linux32 build/res/goldberg/linux64 build/res/goldberg/win && \
cp target/release/partydeck build/ && \
cp LICENSE build/ && cp COPYING.md build/thirdparty.txt && \
cp res/GamingModeLauncher.sh build/ && \
cp res/splitscreen_kwin.js res/splitscreen_kwin_vertical.js build/res && \
cp deps/releases/gbe-linux-release/regular/x64/steamclient.so build/res/goldberg/linux64/steamclient.so && \
cp deps/releases/gbe-linux-release/regular/x32/steamclient.so build/res/goldberg/linux32/steamclient.so && \
cp deps/releases/gbe-win-release/steamclient_experimental/steamclient.dll \
deps/releases/gbe-win-release/steamclient_experimental/steamclient64.dll \
deps/releases/gbe-win-release/steamclient_experimental/GameOverlayRenderer.dll \
deps/releases/gbe-win-release/steamclient_experimental/GameOverlayRenderer64.dll \
build/res/goldberg/win && \
cp deps/releases/umu/umu-run build/bin/ && 
cp deps/gamescope/build-gcc/src/gamescope build/bin/gamescope-kbm
