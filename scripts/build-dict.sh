#!/usr/bin/env bash
# Build the system dictionary (dict.tgz) from Mozc's dictionary, SudachiDict
# and dic-nico-intersection-pixiv, layered in that order: a (reading,
# surface) pair keeps the score of the first source that has it, and a
# later source's own words sort after an earlier source's for the same
# reading. See karukan-cli/README.md for the layering rule.
#
# Layers, in order: Mozc → SudachiDict → [jawiki] → nico-pixiv + Mozc's
# emoticons. jawiki (mozcdic-ut-jawiki, Wikipedia titles) is opt-in: its
# data is CC BY-SA, so a dict.tgz built with it must carry that license.
#
# Usage: scripts/build-dict.sh [--with-jawiki] [--without-nico] [--work DIR] [--out FILE]
#   --with-jawiki   add mozcdic-ut-jawiki (CC BY-SA) after SudachiDict
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
WITH_JAWIKI=0
while [ $# -gt 0 ]; do
    case "$1" in
        --with-jawiki) WITH_JAWIKI=1 ;;
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

# jawiki is Mozc-format too; it sits after the CSVs so it does not fold
# into the Mozc layer (adjacent files of one format form one layer).
if [ "$WITH_JAWIKI" = 1 ]; then
    fetch "https://github.com/utuhiro78/mozcdic-ut-jawiki/raw/main/mozcdic-ut-jawiki.txt.bz2" \
        "$WORK/mozcdic-ut-jawiki.txt.bz2"
    [ -s "$WORK/mozcdic-ut-jawiki.txt" ] || bunzip2 -kf "$WORK/mozcdic-ut-jawiki.txt.bz2"
    INPUTS+=("$WORK/mozcdic-ut-jawiki.txt")
fi

if [ "$WITH_NICO" = 1 ]; then
    fetch "https://raw.githubusercontent.com/ncaq/dic-nico-intersection-pixiv/master/public/dic-nico-intersection-pixiv-google.txt" \
        "$WORK/dic-nico-intersection-pixiv-google.txt"
    INPUTS+=("$WORK/dic-nico-intersection-pixiv-google.txt")
fi

# Mozc's emoticons (BSD-3): `emoticon<TAB>reading reading ...<TAB>category`,
# one line per emoticon, unfolded into the user-dictionary TSV one line per
# (reading, emoticon) so the build reads it like any other source.
fetch "https://raw.githubusercontent.com/google/mozc/$MOZC_REF/src/data/emoticon/emoticon.tsv" \
    "$WORK/emoticon.tsv"
awk -F'\t' 'NR > 1 && $1 != "" && $2 != "" {
    n = split($2, keys, " ")
    for (i = 1; i <= n; i++) if (keys[i] != "") printf "%s\t%s\t顔文字\tmozc-emoticon\n", keys[i], $1
}' "$WORK/emoticon.tsv" > "$WORK/kaomoji.txt"
INPUTS+=("$WORK/kaomoji.txt")

cargo build --release -p karukan-cli --bin karukan-dict
target/release/karukan-dict build "${INPUTS[@]}" -o "$WORK/dict.bin"
tar czf "$OUT" -C "$WORK" dict.bin
echo "wrote $OUT ($(du -h "$OUT" | cut -f1))"
