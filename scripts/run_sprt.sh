#!/usr/bin/env bash
# SPRT test of a new Quant build against a base build, using fastchess.
#
# Install fastchess: https://github.com/Disservin/fastchess
#   git clone https://github.com/Disservin/fastchess && cd fastchess && make -j
#   then put the `fastchess` binary on PATH or set FASTCHESS=/path/to/fastchess.
# You also need an opening book (EPD or PGN), e.g. UHO_Lichess_4852_v1.epd.
#
# Usage: scripts/run_sprt.sh gainer|big-gainer|simplification <new-engine> <base-engine> <book.epd>
# Env:   TC (default 8+0.08), CONCURRENCY (default 10), HASH (default 16), ROUNDS (default 50000;
#        2 games per round), PGN (default sprt.pgn), FASTCHESS
#
# Bounds (process rule 5): gainers [0, 5], simplifications [-5, 0], normalized Elo.
# big-gainer [0, 10]: early Phase 2 features expected to gain a lot (see AGENTS.md).
# Log every result, including failures, in AGENTS.md.
set -euo pipefail

if [ $# -ne 4 ]; then
    sed -n '2,15p' "$0"
    exit 1
fi

case "$1" in
    gainer) bounds="elo0=0 elo1=5.0" ;;
    big-gainer) bounds="elo0=0 elo1=10.0" ;;
    simplification) bounds="elo0=-5.0 elo1=0" ;;
    *) echo "first argument must be 'gainer', 'big-gainer' or 'simplification'" >&2; exit 1 ;;
esac

new=$2 base=$3 book=$4
for f in "$new" "$base" "$book"; do
    [ -e "$f" ] || { echo "not found: $f" >&2; exit 1; }
done

# shellcheck disable=SC2086 # $bounds is intentionally split into two args
exec "${FASTCHESS:-fastchess}" \
    -engine cmd="$new" name=new \
    -engine cmd="$base" name=base \
    -each tc="${TC:-8+0.08}" option.Hash="${HASH:-16}" option.Threads=1 \
    -openings file="$book" format="${book##*.}" order=random \
    -repeat -report penta=true -recover -rounds "${ROUNDS:-50000}" -concurrency "${CONCURRENCY:-10}" \
    -sprt $bounds alpha=0.05 beta=0.05 model=normalized \
    -pgnout file="${PGN:-sprt.pgn}"
