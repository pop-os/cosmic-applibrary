#!/bin/bash

export BUNDLE="cosmic-app-library-nightly.flatpak"
export MANIFEST_PATH="build-aux/com.System76.AppLibrary.Devel.json"
export FLATPAK_MODULE="cosmic-app-library"
export APP_ID="com.System76.AppLibrary.Devel"
export RUNTIME_REPO="https://nightly.gnome.org/gnome-nightly.flatpakrepo"

sudo rm -rf .flatpak-builder/
sudo flatpak-builder --keep-build-dirs --user --disable-rofiles-fuse flatpak_app --repo=repo ${BRANCH:+--default-branch=$BRANCH} ${MANIFEST_PATH} --force-clean --install --system --delete-build-dirs
sudo rm -rf .flatpak-builder/
