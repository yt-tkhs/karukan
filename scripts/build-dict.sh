#!/usr/bin/env bash
# Build the system dictionary (dict.tgz) from Mozc's dictionary, SudachiDict
# and dic-nico-intersection-pixiv, layered in that order: a (reading,
# surface) pair keeps the score of the first source that has it, and a
# later source's own words sort after an earlier source's for the same
# reading. See karukan-cli/README.md for the layering rule.
#
# Usage: scripts/build-dict.sh [--without-nico] [--work DIR] [--out FILE]
#   --without-nico  leave dic-nico-intersection-pixiv out (its data is
#                   scraped from ニコニコ大百科 / ピクシブ百科事典)
#   --work DIR      download and build directory (default: build/dict)
#   --out FILE      the archive to write (default: dict.tgz)
set -euo pipefail

SUDACHI_VERSION=20260723
MOZC_REF=master
WORK=build/dict
OUT=dict.tgz
WITH_NICO=1
while [ $# -gt 0 ]; do
    case "$1" in
        --without-nico) WITH_NICO=0 ;;
        --work) WORK=$2; shift ;;
        --out) OUT=$2; shift ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

cd "$(dirname "$0")/.."
mkdir -p "$WORK"

fetch() { # url dest
    if [ ! -s "$2" ]; then
        echo "downloading $1"
        curl -fL --retry 3 -o "$2" "$1"
    fi
}

for i in 0 1 2 3 4 5 6 7 8 9; do
    fetch "https://raw.githubusercontent.com/google/mozc/$MOZC_REF/src/data/dictionary_oss/dictionary0$i.txt" \
        "$WORK/dictionary0$i.txt"
done
for lex in small_lex core_lex notcore_lex; do
    fetch "http://sudachi.s3-website-ap-northeast-1.amazonaws.com/sudachidict-raw/$SUDACHI_VERSION/$lex.zip" \
        "$WORK/$lex.zip"
    [ -s "$WORK/$lex.csv" ] || unzip -oq "$WORK/$lex.zip" -d "$WORK"
done

INPUTS=("$WORK"/dictionary0?.txt "$WORK/small_lex.csv" "$WORK/core_lex.csv" "$WORK/notcore_lex.csv")
if [ "$WITH_NICO" = 1 ]; then
    fetch "https://raw.githubusercontent.com/ncaq/dic-nico-intersection-pixiv/master/public/dic-nico-intersection-pixiv-google.txt" \
        "$WORK/dic-nico-intersection-pixiv-google.txt"
    INPUTS+=("$WORK/dic-nico-intersection-pixiv-google.txt")
fi

cargo build --release -p karukan-cli --bin karukan-dict
target/release/karukan-dict build "${INPUTS[@]}" -o "$WORK/dict.bin"
tar czf "$OUT" -C "$WORK" dict.bin
echo "wrote $OUT ($(du -h "$OUT" | cut -f1))"
