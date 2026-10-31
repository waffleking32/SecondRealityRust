#!/bin/sh
# Double-click to play: the demo reads assets/ from the folder it is started in.
cd "$(dirname "$0")" && exec ./secondreality "$@"
