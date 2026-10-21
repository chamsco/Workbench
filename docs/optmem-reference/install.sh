#!/bin/sh
# OptMem installer. Run it again to update: it only replaces the tool, and
# `memo init` never touches memories that already exist.
#
#   curl -fsSL https://raw.githubusercontent.com/VictorTaelin/OptMem/main/install.sh | sh

set -e
DIR="$HOME/.optmem"

command -v python3 >/dev/null || {
  echo "OptMem is one Python file, and this machine has no python3." >&2
  echo "Install python3, then run this line again." >&2
  exit 1
}

mkdir -p "$DIR"
curl -fsSL https://raw.githubusercontent.com/VictorTaelin/OptMem/main/memo -o "$DIR/memo.new"
mv "$DIR/memo.new" "$DIR/memo"
chmod +x "$DIR/memo"

exec "$DIR/memo" init
