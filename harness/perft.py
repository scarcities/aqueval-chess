#!/usr/bin/env python3
"""Black-box perft harness for a UCI chess engine.

Drives the engine only through UCI (`position ...` and `go perft N`), so it
keeps working while the engine's internals change. Before the suite it probes
`position startpos`, `position ... moves` with castling, en passant and
promotion; during the suite it re-checks root moves through `position ... moves`.
On a node-count mismatch it bisects with perft divide against a reference
(the built-in Python generator, or any UCI engine with `go perft`, e.g.
Stockfish) down to the exact position and move that is wrong.

Examples:
    python3 harness/perft.py --engine target/release/aqueval
    python3 harness/perft.py --engine target/release/aqueval --max-nodes 0      # full suite
    python3 harness/perft.py --engine target/release/aqueval --only kiwipete
    python3 harness/perft.py --engine target/release/aqueval --reference /usr/bin/stockfish
    python3 harness/perft.py --verify-suite        # check the suite file itself
    python3 harness/perft.py --self-test           # check the harness itself

Exit codes: 0 all passed, 1 at least one wrong result, 2 engine or harness
error (crash, timeout, unreadable output, bad arguments, nothing tested).
"""

import argparse
import collections
import json
import os
import queue
import re
import shlex
import signal
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from refgen import Position  # noqa: E402

DEFAULT_SUITE = os.path.join(HERE, "suites", "standard.epd")
KIWIPETE = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
# A count is plain digits or digit groups of three separated by , _ . or space;
# any text after it must start with a space and a non-digit ("20 (3 ms)").
NUM = r"(\d{1,3}(?:[,_. ]\d{3})+|\d+)(?:\s+\D.*)?"
MOVE_RE = re.compile(r"^\s*([a-h][1-8][a-h][1-8][nbrqNBRQ]?)\s*[:\-=]?\s*" + NUM + "$")
TOTAL_RE = re.compile(r"^\s*(?:nodes searched|total nodes|total|nodes)\s*[:=]?\s*" + NUM + "$",
                      re.IGNORECASE)
CPW_POS4 = "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1"
# Probes run before the suite; expected counts come from refgen at run time.
PROBES = (
    ("position startpos", "startpos", [], 1),
    ("position startpos moves (en passant available)", "startpos",
     ["e2e4", "a7a6", "e4e5", "d7d5"], 2),
    ("position fen moves (castling, capture-promotion)", KIWIPETE,
     ["e1g1", "h3g2", "d5d6", "g2f1q"], 2),
    ("position fen moves (underpromotion)", CPW_POS4, ["g1h1", "b2a1n"], 2),
)

Divide = collections.namedtuple("Divide", "counts dups total elapsed unparsed")


class EngineError(Exception):
    pass


class BisectError(Exception):
    pass


def num(s):
    return int(re.sub(r"[,_. ]", "", s))


def fmt(n):
    return "{:,}".format(n)


def position_cmd(fen, moves):
    base = "position startpos" if fen == "startpos" else "position fen %s" % fen
    return base + (" moves " + " ".join(moves) if moves else "")


def ref_position(fen, moves):
    p = Position() if fen == "startpos" else Position(fen)
    for mv in moves:
        p.push_uci(mv)
    return p


# --------------------------------------------------------------------------
# Suite
# --------------------------------------------------------------------------

def load_suite(path):
    entries, name = [], None
    with open(path) as f:
        for lineno, raw in enumerate(f, 1):
            line = raw.strip()
            if not line:
                continue
            if line.startswith("#"):
                words = line[1:].split()
                name = words[0] if words else None
                continue
            fields = [x.strip() for x in line.split(";")]
            fen, depths = fields[0], {}
            for fld in fields[1:]:
                if not fld:
                    continue
                m = re.match(r"^[Dd](\d+)\s+(\d+)$", fld)
                if not m:
                    raise ValueError("%s:%d: cannot parse %r" % (path, lineno, fld))
                depths[int(m.group(1))] = int(m.group(2))
            if not depths or min(depths) < 1:
                raise ValueError("%s:%d: needs at least one ;D<depth> <nodes> with depth >= 1"
                                 % (path, lineno))
            entries.append({"name": name or "line%d" % lineno, "fen": fen, "depths": depths})
            name = None
    return entries


# --------------------------------------------------------------------------
# UCI engine wrapper
# --------------------------------------------------------------------------

class UciEngine:
    def __init__(self, cmd, timeout, label="engine", log=None, startup_timeout=30):
        self.label, self.timeout, self.log = label, timeout, log
        self.startup_timeout = startup_timeout
        self.prefix = "" if label == "engine" else label + " "
        try:
            self.argv = shlex.split(cmd)
        except ValueError as e:
            raise EngineError("cannot parse %s command %r: %s" % (label, cmd, e))
        if not self.argv:
            raise EngineError("empty %s command" % label)
        self.proc = None
        self.start()

    def start(self):
        try:
            self.proc = subprocess.Popen(
                self.argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                text=True, encoding="utf-8", errors="replace", bufsize=1,
                start_new_session=hasattr(os, "killpg"))
        except OSError as e:
            raise EngineError("cannot start %s %r: %s" % (self.label, " ".join(self.argv), e))
        self.lines = queue.Queue()
        self.dirty = True  # until the handshake succeeds; divide() restarts dirty engines
        threading.Thread(target=self._pump, args=(self.proc, self.lines), daemon=True).start()
        try:
            self.send("uci")
            self.expect(lambda l: l == "uciok", self.startup_timeout, "uciok")
            self.sync(self.startup_timeout)
        except BaseException:  # also Ctrl-C / SIGTERM during the handshake: leave no orphan
            self.kill(grace=0)
            raise
        self.dirty = False

    @staticmethod
    def _pump(proc, q):
        try:
            for line in proc.stdout:
                q.put(line.rstrip("\r\n"))
        except (OSError, ValueError):
            pass
        q.put(None)

    def _log(self, s):
        if self.log:
            self.log.write("%s%s\n" % (self.prefix, s))

    def send(self, cmd):
        self._log("> " + cmd)
        if self.proc is None:
            raise EngineError("%s is not running" % self.label)
        try:
            self.proc.stdin.write(cmd + "\n")
            self.proc.stdin.flush()
        except (OSError, ValueError):
            raise EngineError("%s closed its input (crashed?)" % self.label)

    def readline(self, deadline):
        left = deadline - time.monotonic()
        try:
            if left <= 0:
                raise queue.Empty
            line = self.lines.get(timeout=left)
        except queue.Empty:
            raise EngineError("%s timed out" % self.label)
        if line is None:
            try:
                code = self.proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                code = "unknown"
            raise EngineError("%s exited (code %s)" % (self.label, code))
        self._log("< " + line)
        return line

    def expect(self, pred, timeout, what):
        deadline = time.monotonic() + timeout
        seen = []
        while True:
            try:
                line = self.readline(deadline)
            except EngineError as e:
                tail = [l for l in seen if l.strip()][-4:]
                hint = ("; no %s recognised in its output, last lines: %s"
                        % (what, " | ".join(tail))) if tail else \
                    "; no %s within %gs, it printed nothing" % (what, timeout)
                raise EngineError(str(e) + hint)
            seen.append(line)
            if pred(line.strip()):
                return seen

    def sync(self, timeout=None):
        self.send("isready")
        self.expect(lambda l: l == "readyok", timeout or self.timeout, "readyok")

    def divide(self, fen, moves, depth, timeout=None):
        if self.dirty:
            self.kill()
            try:
                self.start()  # stays dirty if the restart fails
            except EngineError as e:
                raise EngineError("restart after the previous error failed: %s" % e)
        try:
            self.send(position_cmd(fen, moves))
            self.send("go perft %d" % depth)
            start = time.monotonic()
            lines = self.expect(lambda l: not l.startswith("info") and TOTAL_RE.match(l),
                                timeout or self.timeout, "total line (e.g. 'Nodes searched: N')")
            elapsed = time.monotonic() - start
            self.sync()  # drain trailing Time:/NPS: lines
        except EngineError:
            self.dirty = True
            raise
        counts, dups, total, unparsed = {}, [], None, []
        for l in lines:
            l = l.strip()  # parse exactly what expect() matched
            if l.startswith("info") or not l:
                continue
            m = TOTAL_RE.match(l)
            if m:
                total = num(m.group(1))
                continue
            m = MOVE_RE.match(l)
            if m:
                mv = m.group(1).lower()
                if mv in counts:
                    dups.append(mv)
                counts[mv] = counts.get(mv, 0) + num(m.group(2))
            else:
                unparsed.append(l)
        if total is None:
            self.dirty = True
            raise EngineError("%s total line could not be parsed" % self.label)
        return Divide(counts, dups, total, elapsed, unparsed[:3])

    def kill(self, grace=2):
        if self.proc is None:
            return
        try:
            self.proc.stdin.write("quit\n")
            self.proc.stdin.flush()
        except (OSError, ValueError):
            pass
        try:
            self.proc.wait(timeout=grace)
        except subprocess.TimeoutExpired:
            pass
        if hasattr(os, "killpg"):
            try:
                os.killpg(self.proc.pid, signal.SIGKILL)  # also wrapper children
            except (ProcessLookupError, PermissionError):
                pass
        else:
            self.proc.kill()
        try:
            self.proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            pass
        try:  # stdout is left to the reader thread; closing it can block on its lock
            self.proc.stdin.close()
        except (OSError, ValueError):
            pass
        self.proc = None

    close = kill


class BuiltinReference:
    label = "builtin reference"

    def divide(self, fen, moves, depth, timeout=None):
        p = ref_position(fen, moves)
        start = time.monotonic()
        d = p.divide(depth)
        return Divide(d, [], sum(d.values()), time.monotonic() - start, [])

    def close(self):
        pass

# --------------------------------------------------------------------------
# Bisection
# --------------------------------------------------------------------------

def unparsed_note(d):
    return " (lines not understood: %s)" % " | ".join(d.unparsed) if d.unparsed else ""


def check_parsed(d, who, strict):
    if d.total is None:
        raise BisectError("%s printed no total line" % who)
    if d.total > 0 and not d.counts:
        raise BisectError("no divide lines parsed from %s output%s" % (who, unparsed_note(d)))
    s = sum(d.counts.values())
    if s != d.total and (strict or d.unparsed):
        raise BisectError("%s divide sums to %s but its total says %s%s"
                          % (who, fmt(s), fmt(d.total), unparsed_note(d)))


def castling_hint(missing, extra):
    pairs = {"e1g1": "e1h1", "e1c1": "e1a1", "e8g8": "e8h8", "e8c8": "e8a8"}
    if any(m in missing and pairs[m] in extra for m in pairs):
        return ("the engine prints castling as king-takes-rook (e1h1); standard UCI uses the "
                "king's destination (e1g1)")
    return None


def bisect(engine, ref, fen, depth, expected, first_total, out):
    """Walk down the divide tree until the wrong move set or count is found."""
    moves, parent = [], None
    while True:
        out("    divide D%d after [%s]" % (depth, " ".join(moves) or "root"))
        ed = engine.divide(fen, moves, depth)
        rd = ref.divide(fen, moves, depth)
        check_parsed(ed, "engine", strict=False)
        check_parsed(rd, ref.label, strict=True)
        here = ref_position(fen, moves).fen()
        culprit = {"fen": here, "root_fen": fen, "moves": list(moves), "depth": depth}

        if parent is None:
            if rd.total != expected:
                raise BisectError("%s gives %s at the root but the suite expects %s; check the "
                                  "suite entry or the reference" % (ref.label, fmt(rd.total),
                                                                    fmt(expected)))
            if ed.total != first_total:
                out("  >> same command, different answer: the suite run gave %s, the repeat gives "
                    "%s. The perft result depends on earlier commands (state leaking between "
                    "`position` commands, or a perft cache)." % (fmt(first_total), fmt(ed.total)))
                culprit["problem"] = "nondeterministic"
                return culprit
        else:
            pm, pe, pr = parent
            if rd.total != pr:
                raise BisectError("%s is inconsistent: %s counted %s from the parent but %s fresh"
                                  % (ref.label, pm, fmt(pr), fmt(rd.total)))
            if ed.total != pe:
                prev = ref_position(fen, moves[:-1]).fen()
                if ed.total == rd.total:
                    out("  >> wrong count from the parent, correct when set up fresh: the engine "
                        "counts %s %s nodes below the parent position, but %s (correct) after "
                        "`position ... moves`." % (pm, fmt(pe), fmt(ed.total)))
                    out("     Likely causes: make/unmake does not restore some state (castling "
                        "rights, en passant, hash, counters), a perft cache returns stale values, "
                        "or the last-ply bulk count uses a different path than the move list%s."
                        % (" (a candidate here, since the fresh depth is 1)" if depth == 1 else ""))
                    culprit["problem"] = "parent-vs-fresh"
                else:
                    out("  >> inconsistent engine: below the parent it counts %s %s nodes, set up "
                        "fresh it gives %s, the reference says %s. Check the `position ... moves` "
                        "handling and make/unmake." % (pm, fmt(pe), fmt(ed.total), fmt(rd.total)))
                    culprit["problem"] = "inconsistent"
                out("     parent position: %s  (move %s)" % (prev, pm))
                out("     compare: `%s` + `go perft %d` (line %s)  vs  `%s` + `go perft %d` (total)"
                    % (position_cmd(fen, moves[:-1]), depth + 1, pm, position_cmd(fen, moves), depth))
                culprit.update(fen=prev, moves=list(moves[:-1]), move=pm, depth=depth + 1)
                return culprit

        missing = sorted(set(rd.counts) - set(ed.counts))
        extra = sorted(set(ed.counts) - set(rd.counts))
        if ed.dups or missing or extra:
            out("  >> culprit position: %s" % here)
            if moves:
                out("     reached by: %s" % position_cmd(fen, moves))
            if ed.dups:
                out("     engine lists move(s) more than once: %s" % " ".join(sorted(set(ed.dups))))
            if missing:
                out("     engine MISSES legal move(s): %s" % " ".join(missing))
            if extra:
                out("     engine generates ILLEGAL move(s): %s" % " ".join(extra))
            hint = castling_hint(missing, extra)
            if hint:
                out("     hint: " + hint)
            culprit.update(problem="move-list", missing=missing, illegal=extra,
                           duplicates=sorted(set(ed.dups)))
            return culprit

        diff = [m for m in sorted(rd.counts) if ed.counts[m] != rd.counts[m]]
        if not diff:
            out("  >> the engine's divide is correct (sums to %s) but its total line says %s"
                % (fmt(sum(ed.counts.values())), fmt(ed.total)))
            culprit["problem"] = "total-line"
            return culprit
        if depth == 1:
            out("  >> at depth 1 every move counts 1 node, but the engine prints: %s"
                % ", ".join("%s: %s" % (m, ed.counts[m]) for m in diff[:5]))
            culprit["problem"] = "depth1-counts"
            return culprit
        m = diff[0]
        out("     %s: engine %s vs reference %s%s" % (m, fmt(ed.counts[m]), fmt(rd.counts[m]),
            "  (+%d more differing)" % (len(diff) - 1) if len(diff) > 1 else ""))
        parent = (m, ed.counts[m], rd.counts[m])
        moves.append(m)
        depth -= 1


# --------------------------------------------------------------------------
# Main run
# --------------------------------------------------------------------------

def pick_moves_to_recheck(fen, counts):
    """A few root moves to replay through `position ... moves`: castle, ep, promotion, double push."""
    chosen = []
    try:
        legal = ref_position(fen, []).legal_moves()
    except (ValueError, KeyError, IndexError):
        return []  # cannot classify the moves, and a broken FEN would mislead the re-check
    for kind in ("castle", "ep", "promo", "double"):
        for m in legal:
            u = Position.uci(m)
            if u in counts and u not in chosen and (m[3] == kind or (kind == "promo" and m[2])):
                chosen.append(u)
                break
    if len(chosen) < 2:
        for u in sorted(counts):
            if u not in chosen:
                chosen.append(u)
                break
    return chosen[:3]


def run_probes(engine, args, out):
    """Return a list of ("fail" | "error", message); the first probe erroring raises EngineError."""
    results = []
    for i, (title, fen, moves, depth) in enumerate(PROBES):
        want = ref_position(fen, moves).divide(depth)
        cmd = "`%s` + `go perft %d`" % (position_cmd(fen, moves), depth)
        try:
            d = engine.divide(fen, moves, depth, timeout=min(args.timeout, args.probe_timeout))
        except EngineError as e:
            if i == 0:
                raise EngineError("probe '%s' (%s): %s" % (title, cmd, e))
            msg = "probe  %-50s ERROR %s: %s" % (title, cmd, e)
            out(msg)
            results.append(("error", msg))
            continue
        missing = sorted(set(want) - set(d.counts))
        extra = sorted(set(d.counts) - set(want))
        if d.total == sum(want.values()) and not missing and not extra:
            out("probe  %-50s ok" % title)
            continue
        msg = "probe  %-50s FAIL  %s gave %s, expected %s" % (title, cmd, fmt(d.total),
                                                             fmt(sum(want.values())))
        if missing:
            msg += "; missing root moves: %s" % " ".join(missing[:8])
        if extra:
            msg += "; unexpected root moves: %s" % " ".join(extra[:8])
        hint = castling_hint(missing, extra)
        if hint:
            msg += "; " + hint
        out(msg + unparsed_note(d))
        results.append(("fail", msg))
    return results


def probe_reference(ref, args):
    d = ref.divide("startpos", [], 1, timeout=min(args.timeout, args.probe_timeout))
    if d.total != 20 or len(d.counts) != 20:
        raise EngineError("reference gives %s nodes and %d divide lines for `position startpos` + "
                          "`go perft 1` (expected 20 and 20)%s" % (d.total, len(d.counts),
                                                                   unparsed_note(d)))


def run(args, out=print):
    state = {"passed": 0, "failed": 0, "errors": 0, "skipped": 0, "results": [], "aborted": None}
    handles = {"engine": None, "ref": None, "log": None}
    totals = {"nodes": 0, "time": 0.0}
    interrupted = False
    try:
        return _run(args, out, state, handles, totals)
    except BaseException as e:
        interrupted = isinstance(e, (KeyboardInterrupt, SystemExit))
        if state["aborted"] is None:
            state["aborted"] = "interrupted" if interrupted else "harness error: %r" % e
        raise
    finally:
        # Cleanup must finish even if more signals arrive (a second Ctrl-C, SIGTERM).
        old = {}
        for name in ("SIGINT", "SIGTERM", "SIGHUP"):
            if hasattr(signal, name):
                try:
                    old[name] = signal.signal(getattr(signal, name), signal.SIG_IGN)
                except (ValueError, OSError):  # not the main thread
                    pass
        try:
            for key in ("engine", "ref"):
                if handles[key] is not None:
                    handles[key].kill(grace=0 if interrupted else 2)
            for rec in state["results"]:
                rec.setdefault("status", "aborted")
            if args.json:  # before printing: a dead terminal must not cost the JSON
                with open(args.json, "w") as f:
                    json.dump(state, f, indent=2)
            if handles["log"]:
                handles["log"].close()
            try:
                out("")
                out("%d passed, %d failed, %d errors, %d skipped (--max-nodes %s, --max-depth %s)"
                    % (state["passed"], state["failed"], state["errors"], state["skipped"],
                       fmt(args.max_nodes) if args.max_nodes else "-", args.max_depth or "-"))
                if totals["time"] > 0:
                    out("total %s nodes in %.2fs (%s nps wall clock, includes UCI overhead)"
                        % (fmt(totals["nodes"]), totals["time"],
                           fmt(int(totals["nodes"] / totals["time"]))))
                if state["aborted"]:
                    out("RUN ABORTED: %s" % state["aborted"])
            except OSError:
                pass
        finally:
            for name, handler in old.items():
                signal.signal(getattr(signal, name), handler)


def _run(args, out, state, handles, totals):
    def abort(msg):
        out("ERROR: %s" % msg)
        state["aborted"] = msg
        return 2

    def engine_error(rec, label, msg):
        out("%s ERROR  %s" % (label, msg))
        rec.update(status="error", error=msg)
        state["errors"] += 1

    try:
        suite = load_suite(args.suite)
    except (OSError, ValueError) as e:
        return abort("cannot load suite: %s" % e)
    if args.only:
        wanted = [w for w in args.only.split(",") if w]
        unknown = sorted(set(wanted) - {e["name"] for e in suite})
        if unknown:
            return abort("unknown --only name(s): %s (known: %s)"
                         % (", ".join(unknown), ", ".join(e["name"] for e in suite)))
        suite = [e for e in suite if e["name"] in wanted]
    if args.log:
        try:
            handles["log"] = open(args.log, "w", buffering=1)
        except OSError as e:
            return abort("cannot open --log: %s" % e)
    log = handles["log"]

    try:
        engine = handles["engine"] = UciEngine(args.engine, args.timeout, "engine", log,
                                               args.startup_timeout)
    except EngineError as e:
        return abort(str(e))
    if args.reference == "builtin":
        ref = BuiltinReference()
    elif args.reference == "none":
        ref = None
    else:
        try:
            ref = handles["ref"] = UciEngine(args.reference, args.timeout, "ref", log,
                                             args.startup_timeout)
            probe_reference(ref, args)
        except EngineError as e:
            return abort("reference: %s" % e)

    if not args.no_probes:
        try:
            probe_results = run_probes(engine, args, out)
        except EngineError as e:
            return abort("engine does not answer `go perft` correctly: %s" % e)
        for kind, msg in probe_results:
            state["failed" if kind == "fail" else "errors"] += 1
            state["results"].append({"name": "probe", "status": kind, "message": msg})

    consecutive_errors = 0
    for e in suite:
        try:
            root_legal = {Position.uci(m) for m in ref_position(e["fen"], []).legal_moves()}
        except (ValueError, KeyError, IndexError):
            root_legal = None
        checked_moves = checked_names = False
        for depth in sorted(e["depths"]):
            expected = e["depths"][depth]
            if (args.max_depth and depth > args.max_depth) or \
                    (args.max_nodes and expected > args.max_nodes):
                state["skipped"] += 1
                continue
            label = "%-28s D%-2d" % (e["name"], depth)
            rec = {"name": e["name"], "fen": e["fen"], "depth": depth, "expected": expected}
            state["results"].append(rec)
            try:
                d = engine.divide(e["fen"], [], depth)
            except EngineError as err:
                engine_error(rec, label, str(err))
                consecutive_errors += 1
                if consecutive_errors >= 3:
                    return abort("3 engine errors in a row, giving up")
                break
            consecutive_errors = 0
            totals["nodes"] += d.total
            totals["time"] += d.elapsed
            nps = int(d.total / d.elapsed) if d.elapsed > 0 else 0
            rec.update(nodes=d.total, seconds=round(d.elapsed, 3), nps=nps)
            problems = []
            if d.total != expected:
                problems.append("total %s, expected %s (%+d)"
                                % (fmt(d.total), fmt(expected), d.total - expected))
            if d.total > 0 and not d.counts:
                problems.append("no divide lines (move: count) in output" + unparsed_note(d))
            elif sum(d.counts.values()) != d.total:
                problems.append("divide sums to %s but total says %s%s"
                                % (fmt(sum(d.counts.values())), fmt(d.total), unparsed_note(d)))
            if d.dups:
                problems.append("move(s) listed twice: %s" % " ".join(sorted(set(d.dups))))
            if not checked_names and root_legal is not None and d.counts:
                checked_names = True
                missing = sorted(root_legal - set(d.counts))
                extra = sorted(set(d.counts) - root_legal)
                if missing or extra:
                    problems.append("root moves differ from the reference: missing [%s], "
                                    "unexpected [%s]%s" % (
                                        " ".join(missing), " ".join(extra),
                                        "; " + castling_hint(missing, extra)
                                        if castling_hint(missing, extra) else ""))
            if not problems and depth >= 2 and not checked_moves:
                checked_moves = True
                recheck_error = None
                for mv in pick_moves_to_recheck(e["fen"], d.counts):
                    try:
                        sub = engine.divide(e["fen"], [mv], depth - 1)
                    except EngineError as err:
                        recheck_error = "`%s` + `go perft %d` failed: %s" % (
                            position_cmd(e["fen"], [mv]), depth - 1, err)
                        break
                    if sub.total != d.counts[mv]:
                        problems.append(
                            "root divide says %s: %s, but `%s` + `go perft %d` gives %s "
                            "(`position ... moves` handling or make/unmake state is broken)"
                            % (mv, fmt(d.counts[mv]), position_cmd(e["fen"], [mv]), depth - 1,
                               fmt(sub.total)))
                        break
                if recheck_error:
                    engine_error(rec, label, recheck_error)
                    consecutive_errors += 1
                    if consecutive_errors >= 3:
                        return abort("3 engine errors in a row, giving up")
                    break
            if not problems:
                out("%s ok     %15s nodes  %8.3fs  %12s nps"
                    % (label, fmt(d.total), d.elapsed, fmt(nps)))
                rec["status"] = "ok"
                state["passed"] += 1
                continue
            state["failed"] += 1
            rec.update(status="fail", problems=problems)
            out("%s FAIL   %s" % (label, "; ".join(problems)))
            if d.total != expected and ref is not None:
                if isinstance(ref, BuiltinReference) and args.bisect_budget \
                        and expected > args.bisect_budget:
                    out("    (skipping bisection: %s nodes is too slow for the builtin "
                        "reference; pass --reference <stockfish> or raise --bisect-budget)"
                        % fmt(expected))
                else:
                    try:
                        rec["culprit"] = bisect(engine, ref, e["fen"], depth, expected, d.total, out)
                    except EngineError as err:  # the engine or a UCI reference crashed/hung
                        out("    bisection aborted: %s" % err)
                        rec["bisect_error"] = str(err)
                        state["errors"] += 1
                    except Exception as err:  # noqa: BLE001 - report, never crash the run
                        out("    bisection aborted: %s" % err)
                        rec["bisect_error"] = str(err)
            break  # deeper depths of this position would fail too

    if state["errors"]:
        return 2
    if state["failed"]:
        return 1
    if not state["passed"]:
        return abort("nothing was tested (all entries skipped)")
    return 0


def verify_suite(args, out=print):
    """Check suite counts within the limits against the builtin generator or a UCI reference."""
    suite = load_suite(args.suite)
    if args.only:
        wanted = [w for w in args.only.split(",") if w]
        unknown = sorted(set(wanted) - {e["name"] for e in suite})
        if unknown:
            out("ERROR: unknown --only name(s): %s" % ", ".join(unknown))
            return 2
        suite = [e for e in suite if e["name"] in wanted]
    ref = None
    if args.reference not in ("builtin", "none"):
        ref = UciEngine(args.reference, args.timeout, "reference", None, args.startup_timeout)
    bad = skipped = checked = 0
    try:
        if ref:
            probe_reference(ref, args)
        for e in suite:
            for depth, expected in sorted(e["depths"].items()):
                if (args.max_nodes and expected > args.max_nodes) or \
                        (args.max_depth and depth > args.max_depth):
                    skipped += 1
                    continue
                t = time.monotonic()
                got = ref.divide(e["fen"], [], depth).total if ref else Position(e["fen"]).perft(depth)
                checked += 1
                ok = got == expected
                bad += not ok
                out("%-28s D%-2d %s %15s  (%.1fs)%s" % (
                    e["name"], depth, "ok  " if ok else "BAD ", fmt(expected),
                    time.monotonic() - t, "" if ok else "  reference says " + fmt(got)))
    finally:
        if ref:
            ref.close()
    out("\n%d checked, %d wrong, %d skipped (over --max-nodes/--max-depth)" % (checked, bad, skipped))
    if not checked:
        out("ERROR: nothing was checked")
        return 2
    return 1 if bad else 0


def self_test(args, out=print):
    """Run the harness against the mock engine, clean and with injected bugs."""
    import shutil
    import tempfile
    tmp = tempfile.mkdtemp(prefix="perft-selftest-")
    py = "%s -B %s" % (shlex.quote(sys.executable), shlex.quote(os.path.join(HERE, "mock_engine.py")))
    state_file = shlex.quote(os.path.join(tmp, "crashed"))
    cases = (  # bug, --only, max nodes, reference, expected exit, text that must appear
        ("", "", 60000, "builtin", 0, "0 failed, 0 errors"),
        ("ep-pin", "illegal-ep-2", 60000, "builtin", 1, "engine generates ILLEGAL move(s): c4d3"),
        ("ep-pin", "illegal-ep-2", 60000, py, 1, "engine generates ILLEGAL move(s): c4d3"),
        ("no-underpromo", "cpw-pos4", 60000, "builtin", 1,
         "engine MISSES legal move(s): b2a1b b2a1n b2a1r b2b1b b2b1n b2b1r"),
        ("castle-unmake", "kiwipete", 100000, "builtin", 1,
         ">> wrong count from the parent, correct when set up fresh"),
        ("ignore-moves", "startpos", 60000, "builtin", 1,
         "probe  position startpos moves (en passant"),
        ("dup-moves", "startpos", 60000, "builtin", 1, "engine lists move(s) more than once"),
        ("castle-notation", "kiwipete", 60000, "builtin", 1, "king-takes-rook"),
        ("bad-total", "startpos", 60000, "builtin", 2,
         "no total line (e.g. 'Nodes searched: N') recognised"),
        ("crash-pos3", "startpos,cpw-pos3,cpw-pos4", 60000, "builtin", 2,
         "panicked at src/movegen/mod.rs"),
        ("crash-then-dead --state " + state_file, "cpw-pos3,cpw-pos4,cpw-pos5,cpw-pos6", 60000,
         "builtin", 2, "3 engine errors in a row"),
        ("hang-pos3", "cpw-pos3,cpw-pos4", 60000, "builtin", 2, "engine timed out"),
        ("trailing-space", "startpos,kiwipete", 60000, "builtin", 0, "0 failed, 0 errors"),
    )
    ok = True
    try:
        for bug, only, max_nodes, reference, want_rc, want_text in cases:
            out("== self-test: mock engine%s%s ==" % (" with bug " + bug.split()[0] if bug else "",
                                                     "" if reference == "builtin" else ", mock as reference"))
            sub = make_parser().parse_args([])
            sub.engine = py + (" --bug " + bug if bug else "")
            sub.only, sub.max_nodes, sub.reference = only, max_nodes, reference
            sub.timeout, sub.probe_timeout, sub.startup_timeout = (3 if "hang" in bug else 20), 3, 10
            lines = []
            rc = run(sub, out=lambda s: (lines.append(s), out("  " + s)))
            text = "\n".join(lines)
            good = rc == want_rc and want_text in text and "Traceback" not in text
            out("  -> %s (exit %d, expected %d; looked for %r)\n"
                % ("PASS" if good else "FAIL", rc, want_rc, want_text))
            ok &= good
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    out("self-test %s" % ("PASSED" if ok else "FAILED"))
    return 0 if ok else 1


def make_parser():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--engine", help="engine command line (quoted if it has arguments)")
    ap.add_argument("--suite", default=DEFAULT_SUITE)
    ap.add_argument("--only", help="comma-separated position names from the suite")
    ap.add_argument("--max-nodes", type=float, default=5e6,
                    help="skip entries with more expected nodes (default 5e6; 0 = no limit)")
    ap.add_argument("--max-depth", type=int, default=0, help="skip deeper entries (0 = no limit)")
    ap.add_argument("--reference", default="builtin",
                    help="'builtin' (default), 'none', or a UCI engine command with go perft")
    ap.add_argument("--bisect-budget", type=float, default=5e6,
                    help="max nodes the builtin reference will divide (default 5e6; 0 = no limit)")
    ap.add_argument("--timeout", type=float, default=600, help="seconds per go perft (default 600)")
    ap.add_argument("--startup-timeout", type=float, default=30,
                    help="seconds to wait for uciok/readyok at engine start (default 30)")
    ap.add_argument("--probe-timeout", type=float, default=30,
                    help="seconds for each startup probe (default 30)")
    ap.add_argument("--no-probes", action="store_true", help="skip the position/moves probes")
    ap.add_argument("--json", help="write machine-readable results here")
    ap.add_argument("--log", help="write the full UCI conversation here")
    ap.add_argument("--verify-suite", action="store_true",
                    help="check the suite counts with the reference instead of testing an engine")
    ap.add_argument("--self-test", action="store_true", help="test the harness with mock_engine.py")
    return ap


def main():
    ap = make_parser()
    args = ap.parse_args()

    def usage_error(msg):
        if args.json:  # do not leave a stale result file behind
            try:
                with open(args.json, "w") as f:
                    json.dump({"passed": 0, "failed": 0, "errors": 0, "skipped": 0,
                               "results": [], "aborted": "usage error: " + msg}, f, indent=2)
            except OSError:
                pass
        ap.error(msg)

    for name in ("max_nodes", "bisect_budget"):
        v = getattr(args, name)
        if v != v or v in (float("inf"), float("-inf")) or v < 0:
            usage_error("--%s must be a finite number >= 0" % name.replace("_", "-"))
        setattr(args, name, int(v))
    for sig in ("SIGTERM", "SIGHUP"):  # run finally blocks so engines are killed
        if hasattr(signal, sig) and signal.getsignal(getattr(signal, sig)) is not signal.SIG_IGN:
            signal.signal(getattr(signal, sig), lambda *_: sys.exit(2))  # (keeps nohup working)
    try:
        if args.verify_suite:
            return verify_suite(args)
        if args.self_test:
            return self_test(args)
        if not args.engine or not args.engine.strip():
            usage_error("--engine is required")
        return run(args)
    except KeyboardInterrupt:
        print("interrupted")
        return 2
    except (OSError, ValueError, EngineError) as e:
        print("ERROR: %s" % e)
        return 2
    except Exception:  # a harness bug must never look like a perft mismatch (exit 1)
        import traceback
        traceback.print_exc()
        print("INTERNAL ERROR in the harness (exit 2); please report the traceback above")
        return 2


if __name__ == "__main__":
    sys.exit(main())
