#!/usr/bin/env bash
# Rough Elo estimate: Quant vs Stockfish with UCI_LimitStrength at several UCI_Elo levels.
# This is an approximation against limited Stockfish, not a CCRL rating.
#
# Usage: scripts/elo_gauntlet.sh <quant-binary> <stockfish-binary> <book.epd>
# Env:   LEVELS (default "1400 1700 2000 2300"), TC (default 10+0.1), GAMES (default 200),
#        CONCURRENCY (default 5), FASTCHESS (default fastchess), OUT (default gauntlet)
#
# For each level prints Quant's score and the Elo difference fastchess reports;
# estimated Quant Elo = level + difference.
set -euo pipefail

if [ $# -ne 3 ]; then
    sed -n '2,11p' "$0"
    exit 1
fi
quant=$1 sf=$2 book=$3
for f in "$quant" "$sf" "$book"; do
    [ -e "$f" ] || { echo "not found: $f" >&2; exit 1; }
done
out=${OUT:-gauntlet}
mkdir -p "$out"

for level in ${LEVELS:-1400 1700 2000 2300}; do
    log="$out/sf$level.log"
    "${FASTCHESS:-fastchess}" \
        -engine cmd="$quant" name=Quant \
        -engine cmd="$sf" name="SF$level" option.UCI_LimitStrength=true option.UCI_Elo="$level" \
        -each tc="${TC:-10+0.1}" option.Hash=16 option.Threads=1 \
        -openings file="$book" format="${book##*.}" order=random \
        -repeat -recover -rounds $(( ${GAMES:-200} / 2 )) -concurrency "${CONCURRENCY:-5}" \
        -pgnout file="$out/sf$level.pgn" > "$log" 2>&1
    # Last summary block of the run: "Elo: <diff> +/- <err>" and "Games: ... Points: <p> (<pct> %)"
    elo=$(grep -E '^Elo:' "$log" | tail -1 | awk '{print $2, $3, $4}' | tr -d ,)
    games=$(grep -E '^Games:' "$log" | tail -1)
    diff=${elo%% *}
    est=$(awk -v l="$level" -v d="$diff" 'BEGIN { if (d ~ /^-?[0-9.]+$/) printf "%d", l + d; else print "n/a" }')
    echo "UCI_Elo $level | $games | Elo diff $elo | estimate $est"
done
