#!/usr/bin/env python3
"""Temporary UCI front end for an engine that only has a command-line perft.

Until Quant speaks UCI, this lets the harness test its move generator through
`quant divide <depth> <fen>`:

    python3 harness/perft.py --engine "python3 harness/cli_shim.py target/release/quant"

`position ... moves ...` is resolved to a FEN with refgen.py before calling the
engine, so this does NOT test the engine's own `moves` handling; once the
engine has UCI, test the binary directly instead.
"""

import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from refgen import Position  # noqa: E402

START = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"


def resolve(tok):
    rest = tok[1:]
    if rest and rest[0] == "startpos":
        fen, rest = START, rest[1:]
    elif rest and rest[0] == "fen":
        rest = rest[1:]
        cut = rest.index("moves") if "moves" in rest else len(rest)
        fen, rest = " ".join(rest[:cut]), rest[cut:]
    else:
        raise ValueError("cannot parse %r" % " ".join(tok))
    if rest and rest[0] == "moves" and len(rest) > 1:
        p = Position(fen)
        for mv in rest[1:]:
            p.push_uci(mv)
        fen = p.fen()
    return fen


def main():
    if len(sys.argv) < 2:
        sys.exit("usage: cli_shim.py <engine binary> [extra args before 'divide']")
    engine = sys.argv[1:]
    fen = START
    for line in sys.stdin:
        tok = line.split()
        if not tok:
            continue
        if tok[0] == "uci":
            print("id name cli-shim(%s)\nuciok" % os.path.basename(engine[0]), flush=True)
        elif tok[0] == "isready":
            print("readyok", flush=True)
        elif tok[0] == "position":
            try:
                fen = resolve(tok)
            except ValueError as e:
                print("info string %s" % e, flush=True)
        elif tok[0] == "go" and len(tok) >= 3 and tok[1] == "perft":
            r = subprocess.run(engine + ["divide", tok[2], fen], stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, text=True, errors="replace")
            sys.stdout.write(r.stdout)
            sys.stdout.flush()
            if r.returncode != 0 or "Nodes searched" not in r.stdout:
                # Exit so the harness reports a crash (with the output above) instead of
                # waiting for a total line that will never come; it restarts the shim.
                print("cli_shim: engine exited with code %d" % r.returncode, flush=True)
                sys.exit(r.returncode or 1)
        elif tok[0] == "quit":
            break


if __name__ == "__main__":
    main()
