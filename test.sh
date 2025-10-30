#!/bin/bash
echo 49fb0eabf3092465983a2ab7d1b381e85ee9aa92 > ./current_version_commit_hash.txt
rm ./_assets/kr1-desktop/images/fullhd/screen_map_flags-1.png
rm ./_assets/kr1-desktop/images/fullhd/upgrades-1.png
rm ./_assets/kr1-desktop/images/fullhd/kr4_kr5_encyclopedia_creeps-1.png
cargo run
