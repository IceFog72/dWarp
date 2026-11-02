#!/usr/bin/env bash
# Configure the fork's merge driver and enable rerere after cloning.
# Paths marked merge=openwarp-ours in .gitattributes retain the local version.
# Rerere records conflict resolutions for reuse in later upstream merges.

git config merge.openwarp-ours.name "Always keep dWarp version (custom driver)"
git config merge.openwarp-ours.driver true
git config rerere.enabled true
git config rerere.autoupdate true

echo "dWarp merge drivers and rerere configured."
echo "  rerere.enabled        = $(git config --get rerere.enabled)"
echo "  rerere.autoupdate     = $(git config --get rerere.autoupdate)"
echo "  merge.openwarp-ours   = $(git config --get merge.openwarp-ours.driver)"
