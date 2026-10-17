"""Small, independent reference move generator (pure Python, stdlib only).

Used by the perft harness to locate the exact move where the engine goes wrong
when no external reference engine is available. It is deliberately simple
(0x88 mailbox, pseudo-legal generation + "king left in check" filter) so that it
is easy to trust; it is slow (~100k nodes/s), so it is only used for shallow
divides during bisection and for the harness self-test.
"""

FILES = "abcdefgh"
KNIGHT = (33, 31, 18, 14, -33, -31, -18, -14)
KING = (1, -1, 16, -16, 17, 15, -17, -15)
BISHOP = (17, 15, -17, -15)
ROOK = (1, -1, 16, -16)

# Castling-rights mask per square: moving from/to these squares clears rights.
CASTLE_MASK = {0x04: "KQ", 0x07: "K", 0x00: "Q", 0x74: "kq", 0x77: "k", 0x70: "q"}


def sq_name(sq):
    return FILES[sq & 7] + str((sq >> 4) + 1)


def parse_sq(s):
    return (int(s[1]) - 1) * 16 + FILES.index(s[0])


def white(p):
    return p.isupper()


class Position:
    def __init__(self, fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"):
        parts = fen.split()
        if len(parts) < 4:
            raise ValueError("bad FEN: %r" % fen)
        self.board = [""] * 128
        rows = parts[0].split("/")
        if len(rows) != 8:
            raise ValueError("bad FEN board: %r" % fen)
        for i, row in enumerate(rows):
            rank = 7 - i
            f = 0
            for ch in row:
                if ch.isdigit():
                    f += int(ch)
                else:
                    self.board[rank * 16 + f] = ch
                    f += 1
        self.white = parts[1] == "w"
        self.castling = "" if parts[2] == "-" else parts[2]
        self.ep = None if parts[3] == "-" else parse_sq(parts[3])
        self.kings = {}
        for sq in range(128):
            if self.board[sq] in ("K", "k"):
                if (self.board[sq] == "K") in self.kings:
                    raise ValueError("FEN has more than one %s king: %r"
                                     % ("white" if self.board[sq] == "K" else "black", fen))
                self.kings[self.board[sq] == "K"] = sq
        self._validate(fen)

    def _validate(self, fen):
        """Refuse positions the reference would otherwise silently misinterpret."""
        b = self.board
        if len(self.kings) != 2:
            raise ValueError("FEN needs exactly one king per side: %r" % fen)
        if any(b[sq] in ("P", "p") for sq in list(range(8)) + list(range(0x70, 0x78))):
            raise ValueError("FEN has a pawn on the first or last rank: %r" % fen)
        home = {"K": (0x04, 0x07, "K", "R"), "Q": (0x04, 0x00, "K", "R"),
                "k": (0x74, 0x77, "k", "r"), "q": (0x74, 0x70, "k", "r")}
        for c in self.castling:
            if c not in home:
                raise ValueError("bad castling field in FEN: %r" % fen)
            ksq, rsq, k, r = home[c]
            if b[ksq] != k or b[rsq] != r:
                raise ValueError("castling right %s without king and rook on their home squares: %r"
                                 % (c, fen))
        if self.ep is not None:
            # white to move: ep square on rank 6, black pawn just moved from rank 7 to 5
            rank, pawn, step = (5, "p", -16) if self.white else (2, "P", 16)
            if self.ep >> 4 != rank or b[self.ep + step] != pawn or b[self.ep] \
                    or b[self.ep - step]:
                raise ValueError("inconsistent en passant square in FEN: %r" % fen)
        if self.attacked(self.kings[not self.white], self.white):
            raise ValueError("side not to move is in check: %r" % fen)

    # ---- attacks -------------------------------------------------------
    def attacked(self, sq, by_white):
        b = self.board
        # pawns
        if by_white:
            for d in (-15, -17):
                s = sq + d
                if not s & 0x88 and b[s] == "P":
                    return True
        else:
            for d in (15, 17):
                s = sq + d
                if not s & 0x88 and b[s] == "p":
                    return True
        n, k = ("N", "K") if by_white else ("n", "k")
        for d in KNIGHT:
            s = sq + d
            if not s & 0x88 and b[s] == n:
                return True
        for d in KING:
            s = sq + d
            if not s & 0x88 and b[s] == k:
                return True
        bq = ("B", "Q") if by_white else ("b", "q")
        rq = ("R", "Q") if by_white else ("r", "q")
        for dirs, pcs in ((BISHOP, bq), (ROOK, rq)):
            for d in dirs:
                s = sq + d
                while not s & 0x88:
                    p = b[s]
                    if p:
                        if p in pcs:
                            return True
                        break
                    s += d
        return False

    def in_check(self):
        return self.attacked(self.kings[self.white], not self.white)

    # ---- generation ----------------------------------------------------
    def pseudo_moves(self):
        b = self.board
        us = self.white
        out = []
        for sq in range(128):
            if sq & 0x88:
                continue
            p = b[sq]
            if not p or white(p) != us:
                continue
            t = p.upper()
            if t == "P":
                fwd = 16 if us else -16
                start_rank = 1 if us else 6
                promo_rank = 7 if us else 0
                to = sq + fwd
                if not to & 0x88 and not b[to]:
                    if to >> 4 == promo_rank:
                        for pr in "qrbn":
                            out.append((sq, to, pr, None))
                    else:
                        out.append((sq, to, None, None))
                        to2 = to + fwd
                        if sq >> 4 == start_rank and not b[to2]:
                            out.append((sq, to2, None, "double"))
                for d in ((15, 17) if us else (-15, -17)):
                    to = sq + d
                    if to & 0x88:
                        continue
                    q = b[to]
                    if q and white(q) != us:
                        if to >> 4 == promo_rank:
                            for pr in "qrbn":
                                out.append((sq, to, pr, None))
                        else:
                            out.append((sq, to, None, None))
                    elif to == self.ep:
                        out.append((sq, to, None, "ep"))
            elif t in "NK":
                for d in (KNIGHT if t == "N" else KING):
                    to = sq + d
                    if to & 0x88:
                        continue
                    q = b[to]
                    if not q or white(q) != us:
                        out.append((sq, to, None, None))
            else:
                dirs = BISHOP if t == "B" else ROOK if t == "R" else BISHOP + ROOK
                for d in dirs:
                    to = sq + d
                    while not to & 0x88:
                        q = b[to]
                        if q:
                            if white(q) != us:
                                out.append((sq, to, None, None))
                            break
                        out.append((sq, to, None, None))
                        to += d
        # castling
        them = not us
        if us:
            if "K" in self.castling and b[0x05] == b[0x06] == "" and b[0x07] == "R" \
                    and not self.attacked(0x04, them) and not self.attacked(0x05, them) \
                    and not self.attacked(0x06, them):
                out.append((0x04, 0x06, None, "castle"))
            if "Q" in self.castling and b[0x01] == b[0x02] == b[0x03] == "" and b[0x00] == "R" \
                    and not self.attacked(0x04, them) and not self.attacked(0x03, them) \
                    and not self.attacked(0x02, them):
                out.append((0x04, 0x02, None, "castle"))
        else:
            if "k" in self.castling and b[0x75] == b[0x76] == "" and b[0x77] == "r" \
                    and not self.attacked(0x74, them) and not self.attacked(0x75, them) \
                    and not self.attacked(0x76, them):
                out.append((0x74, 0x76, None, "castle"))
            if "q" in self.castling and b[0x71] == b[0x72] == b[0x73] == "" and b[0x70] == "r" \
                    and not self.attacked(0x74, them) and not self.attacked(0x73, them) \
                    and not self.attacked(0x72, them):
                out.append((0x74, 0x72, None, "castle"))
        return out

    def make(self, m):
        frm, to, promo, flag = m
        b = self.board
        p = b[frm]
        undo = (m, b[to], self.castling, self.ep, dict(self.kings))
        captured_ep = None
        b[to] = (promo.upper() if self.white else promo) if promo else p
        b[frm] = ""
        if flag == "ep":
            cap = to - 16 if self.white else to + 16
            captured_ep = b[cap]
            b[cap] = ""
        elif flag == "castle":
            if to & 7 == 6:
                b[to - 1], b[to + 1] = b[to + 1], ""
            else:
                b[to + 1], b[to - 2] = b[to - 2], ""
        if p in ("K", "k"):
            self.kings[self.white] = to
        if self.castling:
            for s in (frm, to):
                if s in CASTLE_MASK:
                    for c in CASTLE_MASK[s]:
                        self.castling = self.castling.replace(c, "")
        self.ep = (frm + to) // 2 if flag == "double" else None
        self.white = not self.white
        return undo + (captured_ep,)

    def unmake(self, undo):
        (frm, to, promo, flag), captured, castling, ep, kings, captured_ep = undo
        b = self.board
        self.white = not self.white
        p = b[to]
        if promo:
            p = "P" if self.white else "p"
        b[frm] = p
        b[to] = captured
        if flag == "ep":
            b[to - 16 if self.white else to + 16] = captured_ep
        elif flag == "castle":
            if to & 7 == 6:
                b[to + 1], b[to - 1] = b[to - 1], ""
            else:
                b[to - 2], b[to + 1] = b[to + 1], ""
        self.castling = castling
        self.ep = ep
        self.kings = kings

    def legal_moves(self):
        out = []
        for m in self.pseudo_moves():
            u = self.make(m)
            if not self.attacked(self.kings[not self.white], self.white):
                out.append(m)
            self.unmake(u)
        return out

    # ---- helpers -------------------------------------------------------
    def fen(self):
        rows = []
        for rank in range(7, -1, -1):
            row, empty = "", 0
            for f in range(8):
                p = self.board[rank * 16 + f]
                if p:
                    row += (str(empty) if empty else "") + p
                    empty = 0
                else:
                    empty += 1
            rows.append(row + (str(empty) if empty else ""))
        ep = sq_name(self.ep) if self.ep is not None else "-"
        return "%s %s %s %s 0 1" % ("/".join(rows), "w" if self.white else "b",
                                    self.castling or "-", ep)

    @staticmethod
    def uci(m):
        return sq_name(m[0]) + sq_name(m[1]) + (m[2] or "")

    def push_uci(self, s):
        for m in self.legal_moves():
            if self.uci(m) == s:
                self.make(m)
                return
        raise ValueError("illegal move %s" % s)

    def perft(self, depth):
        if depth == 0:
            return 1
        moves = self.legal_moves()
        if depth == 1:
            return len(moves)
        n = 0
        for m in moves:
            u = self.make(m)
            n += self.perft(depth - 1)
            self.unmake(u)
        return n

    def divide(self, depth):
        res = {}
        for m in self.legal_moves():
            u = self.make(m)
            res[self.uci(m)] = self.perft(depth - 1)
            self.unmake(u)
        return res
