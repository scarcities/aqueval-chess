#!/usr/bin/env python3
"""Minimal UCI engine built on refgen.py, used only to test the harness.

Speaks just enough UCI for perft: uci, isready, position, go perft N, quit.
`--bug NAME` injects a known bug so the self-test can check that the harness
detects it and reports the right cause:
    ep-pin         allows en passant captures that leave the own king in check
    no-underpromo  never generates promotions to rook, bishop or knight
    castle-unmake  unmaking a king move does not restore castling rights
    ignore-moves   ignores the `moves ...` part of the position command
    dup-moves      lists the first legal move twice
    castle-notation prints castling as king-takes-rook (e1h1)
    bad-total      prints the total as `Perft = N` (a format the harness does not know)
    crash-pos3     panics (exit 101) on `go perft` >= 2 in CPW position 3
    crash-then-dead  like crash-pos3, and every later start (marker file given by
                   `--state PATH`) prints a banner and exits without speaking UCI
"""

import os
import sys
import time

POS3 = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from refgen import Position  # noqa: E402

START = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"


class BuggyPosition(Position):
    bug = None

    def legal_moves(self):
        if self.bug == "no-underpromo":
            return [m for m in Position.legal_moves(self) if m[2] in (None, "q")]
        if self.bug == "dup-moves":
            moves = Position.legal_moves(self)
            return moves[:1] + moves
        if self.bug == "ep-pin":
            out = []
            for m in self.pseudo_moves():
                u = self.make(m)
                if m[3] == "ep" or not self.attacked(self.kings[not self.white], self.white):
                    out.append(m)
                self.unmake(u)
            return out
        return Position.legal_moves(self)

    def unmake(self, undo):
        frm, to, promo, flag = undo[0]
        king_move = self.board[to] in ("K", "k") and flag != "castle"
        lost = self.castling
        Position.unmake(self, undo)
        if self.bug == "castle-unmake" and king_move:
            self.castling = lost

    def divide_list(self, depth):
        res = []
        for m in self.legal_moves():
            u = self.make(m)
            name = self.uci(m)
            if self.bug == "castle-notation" and m[3] == "castle":
                name = name[:2] + ("h" if name[2] == "g" else "a") + name[3]
            res.append((name, self.perft(depth - 1)))
            self.unmake(u)
        return res


def set_position(tok):
    """Parse `position ...`; moves are applied with the correct generator."""
    if len(tok) < 2:
        return None
    rest = tok[1:]
    if rest[0] == "startpos":
        fen, rest = START, rest[1:]
    elif rest[0] == "fen":
        rest = rest[1:]
        cut = rest.index("moves") if "moves" in rest else len(rest)
        fen, rest = " ".join(rest[:cut]), rest[cut:]
    else:
        return None
    p = Position(fen)
    if rest and rest[0] == "moves" and BuggyPosition.bug != "ignore-moves":
        for mv in rest[1:]:
            p.push_uci(mv)
    return BuggyPosition(p.fen())


def main():
    if "--bug" in sys.argv:
        BuggyPosition.bug = sys.argv[sys.argv.index("--bug") + 1]
    state = sys.argv[sys.argv.index("--state") + 1] if "--state" in sys.argv else None
    if BuggyPosition.bug == "crash-then-dead" and state and os.path.exists(state):
        print("Quant Chess Engine v0.1.0 (no UCI)", flush=True)
        return
    pos = BuggyPosition(START)
    for line in sys.stdin:
        tok = line.split()
        if not tok:
            continue
        cmd = tok[0]
        if cmd == "uci":
            print("id name refgen-mock\nid author harness\nuciok", flush=True)
        elif cmd == "isready":
            print("readyok", flush=True)
        elif cmd == "position":
            try:
                new = set_position(tok)
            except ValueError as e:
                new = None
                print("info string bad position: %s" % e, flush=True)
            if new is not None:
                pos = new
        elif cmd == "go" and len(tok) >= 3 and tok[1] == "perft":
            depth = max(int(tok[2]), 1)
            if BuggyPosition.bug in ("crash-pos3", "crash-then-dead") and depth >= 2 \
                    and pos.fen() == POS3:
                if state:
                    open(state, "w").close()
                print("thread 'main' panicked at src/movegen/mod.rs:311:17:\n"
                      "index out of bounds: the len is 64 but the index is 64", flush=True)
                sys.exit(101)
            t = time.monotonic()
            d = pos.divide_list(depth)
            ms = int((time.monotonic() - t) * 1000)
            total = sum(n for _, n in d)
            for mv, n in d:
                print("%s: %d" % (mv, n))
            if BuggyPosition.bug == "bad-total":
                print("\nPerft = %d" % total, flush=True)
            else:
                print("\nNodes searched: %d\nTime: %d ms\nNPS: %d"
                      % (total, ms, total * 1000 // max(ms, 1)), flush=True)
        elif cmd == "quit":
            break


if __name__ == "__main__":
    main()
