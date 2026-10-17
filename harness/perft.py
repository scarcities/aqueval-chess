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
    python3 harness/perft.py --engine target/release/quant
    python3 harness/perft.py --engine target/release/quant --max-nodes 0      # full suite
    python3 harness/perft.py --engine target/release/quant --only kiwipete
    python3 harness/perft.py --engine target/release/quant --reference /usr/bin/stockfish
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
MOVE_RE = re.compile(r"^\s*([a-h][1-8][a-h][1-8][nbrqNBRQ]?)\s*[:\-=]?\s*(\d[\d,_]*)(?:\s.*)?$")
TOTAL_RE = re.compile(r"^\s*(?:nodes searched|total nodes|total|nodes)\s*[:=]?\s*(\d[\d,_]*)(?:\s.*)?$",
                      re.IGNORECASE)
# Probes run before the suite; expected counts come from refgen at run time.
PROBES = (
    ("position startpos", "startpos", [], 1),
    ("position startpos moves (en passant available)", "startpos",
     ["e2e4", "a7a6", "e4e5", "d7d5"], 2),
    ("position fen moves (castling, capture-promotion)", KIWIPETE,
     ["e1g1", "h3g2", "d5d6", "g2f1q"], 2),
)

Divide = collections.namedtuple("Divide", "counts dups total elapsed unparsed")


class EngineError(Exception):
    pass


class BisectError(Exception):
    pass


def num(s):
    return int(s.replace(",", "").replace("_", ""))


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
        self.dirty = False
        threading.Thread(target=self._pump, args=(self.proc, self.lines), daemon=True).start()
        try:
            self.send("uci")
            self.expect(lambda l: l == "uciok", self.startup_timeout, "uciok")
            self.sync(self.startup_timeout)
        except EngineError:
            self.kill()
            raise

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
                        % (what, " | ".join(tail))) if tail else "; it printed nothing"
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
            self.start()
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
            if l.startswith("info") or not l.strip():
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
        return Divide(counts, dups, total, elapsed, unparsed[:3])

    def kill(self):
        if self.proc is None:
            return
        try:
            self.proc.stdin.write("quit\n")
            self.proc.stdin.flush()
        except (OSError, ValueError):
            pass
        try:
            self.proc.wait(timeout=2)
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
        for stream in (self.proc.stdin, self.proc.stdout):
            try:
                stream.close()
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

def check_parsed(d, who, strict):
    if d.total is None:
        raise BisectError("%s printed no total line" % who)
    if d.total > 0 and not d.counts:
        raise BisectError("no divide lines parsed from %s output%s" % (
            who, " (unparsed: %s)" % " | ".join(d.unparsed) if d.unparsed else ""))
    if strict and sum(d.counts.values()) != d.total:
        raise BisectError("%s divide sums to %s but its total says %s"
                          % (who, fmt(sum(d.counts.values())), fmt(d.total)))


def castling_hint(missing, extra):
    pairs = {"e1g1": "e1h1", "e1c1": "e1a1", "e8g8": "e8h8", "e8c8": "e8a8"}
    if any(m in missing and pairs[m] in extra for m in pairs):
        return ("     hint: the engine prints castling as king-takes-rook (e1h1); standard UCI "
                "uses the king's destination (e1g1)")
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
                    out("  >> state not restored: counted from the parent position the engine gives "
                        "%s %s nodes, but set up fresh with `position ... moves` it gives %s "
                        "(correct)." % (pm, fmt(pe), fmt(ed.total)))
                    out("     Make/unmake does not restore some state (castling rights, en passant, "
                        "hash, counters) or a perft cache returns stale values, somewhere in the "
                        "subtree of the parent position.")
                    culprit["problem"] = "state-not-restored"
                else:
                    out("  >> inconsistent engine: from the parent it counts %s %s nodes, set up "
                        "fresh it gives %s, the reference says %s. Check the `position ... moves` "
                        "handling and make/unmake." % (pm, fmt(pe), fmt(ed.total), fmt(rd.total)))
                    culprit["problem"] = "inconsistent"
                out("     parent position: %s  (move %s)" % (prev, pm))
                out("     reproduce: %s / go perft %d" % (position_cmd(fen, moves), depth))
                culprit.update(fen=prev, moves=list(moves[:-1]), move=pm)
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
                out(hint)
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
    except (ValueError, KeyError):
        legal = []
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
    """Return list of failure messages; raises EngineError if perft is not answered at all."""
    failures = []
    for title, fen, moves, depth in PROBES:
        p = ref_position(fen, moves)
        want = p.divide(depth)
        d = engine.divide(fen, moves, depth, timeout=min(args.timeout, args.probe_timeout))
        missing = sorted(set(want) - set(d.counts))
        extra = sorted(set(d.counts) - set(want))
        if d.total == sum(want.values()) and not missing and not extra:
            out("probe  %-50s ok" % title)
            continue
        msg = "probe  %-50s FAIL  `%s` + `go perft %d` gave %s, expected %s" % (
            title, position_cmd(fen, moves), depth, fmt(d.total), fmt(sum(want.values())))
        if missing:
            msg += "; missing root moves: %s" % " ".join(missing[:8])
        if extra:
            msg += "; unexpected root moves: %s" % " ".join(extra[:8])
        out(msg)
        failures.append(msg)
    return failures


def run(args, out=print):
    suite = load_suite(args.suite)
    if args.only:
        wanted = [w for w in args.only.split(",") if w]
        unknown = sorted(set(wanted) - {e["name"] for e in suite})
        if unknown:
            out("ERROR: unknown --only name(s): %s (known: %s)"
                % (", ".join(unknown), ", ".join(e["name"] for e in suite)))
            return 2
        suite = [e for e in suite if e["name"] in wanted]

    state = {"passed": 0, "failed": 0, "errors": 0, "skipped": 0, "results": [], "aborted": None}
    log = open(args.log, "w", buffering=1) if args.log else None
    engine = ref = None
    total_nodes, total_time = 0, 0.0

    def abort(msg):
        out("ERROR: %s" % msg)
        state["aborted"] = msg
        return 2

    try:
        try:
            engine = UciEngine(args.engine, args.timeout, "engine", log)
        except EngineError as e:
            return abort(str(e))
        if args.reference == "builtin":
            ref = BuiltinReference()
        elif args.reference != "none":
            try:
                ref = UciEngine(args.reference, args.timeout, "ref", log)
            except EngineError as e:
                return abort(str(e))

        if not args.no_probes:
            try:
                fails = run_probes(engine, args, out)
            except EngineError as e:
                return abort("engine does not answer `go perft` correctly: %s" % e)
            for msg in fails:
                state["failed"] += 1
                state["results"].append({"name": "probe", "status": "fail", "message": msg})

        consecutive_errors = 0
        for e in suite:
            checked_moves = False
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
                    out("%s ERROR  %s" % (label, err))
                    rec.update(status="error", error=str(err))
                    state["errors"] += 1
                    consecutive_errors += 1
                    if consecutive_errors >= 3:
                        return abort("3 engine errors in a row, giving up")
                    break
                consecutive_errors = 0
                total_nodes += d.total
                total_time += d.elapsed
                nps = int(d.total / d.elapsed) if d.elapsed > 0 else 0
                rec.update(nodes=d.total, seconds=round(d.elapsed, 3), nps=nps)
                problems = []
                if d.total != expected:
                    problems.append("total %s, expected %s (%+d)"
                                    % (fmt(d.total), fmt(expected), d.total - expected))
                if not d.counts:
                    problems.append("no divide lines (move: count) in output")
                elif sum(d.counts.values()) != d.total:
                    problems.append("divide sums to %s but total says %s"
                                    % (fmt(sum(d.counts.values())), fmt(d.total)))
                if d.dups:
                    problems.append("move(s) listed twice: %s" % " ".join(sorted(set(d.dups))))
                if not problems and depth >= 2 and not checked_moves:
                    checked_moves = True
                    for mv in pick_moves_to_recheck(e["fen"], d.counts):
                        try:
                            sub = engine.divide(e["fen"], [mv], depth - 1)
                        except EngineError as err:
                            problems.append("`%s` + `go perft %d` failed: %s"
                                            % (position_cmd(e["fen"], [mv]), depth - 1, err))
                            break
                        if sub.total != d.counts[mv]:
                            problems.append(
                                "root divide says %s: %s, but `%s` + `go perft %d` gives %s "
                                "(`position ... moves` handling or make/unmake state is broken)"
                                % (mv, fmt(d.counts[mv]), position_cmd(e["fen"], [mv]), depth - 1,
                                   fmt(sub.total)))
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
                    if isinstance(ref, BuiltinReference) and expected > args.bisect_budget:
                        out("    (skipping bisection: %s nodes is too slow for the builtin "
                            "reference; pass --reference <stockfish> or raise --bisect-budget)"
                            % fmt(expected))
                    else:
                        try:
                            rec["culprit"] = bisect(engine, ref, e["fen"], depth, expected,
                                                    d.total, out)
                        except Exception as err:  # noqa: BLE001 - report, never crash the run
                            out("    bisection aborted: %s" % err)
                break  # deeper depths of this position would fail too
    finally:
        for proc in (engine, ref):
            if proc is not None:
                proc.close()
        out("")
        out("%d passed, %d failed, %d errors, %d skipped (--max-nodes %s, --max-depth %s)"
            % (state["passed"], state["failed"], state["errors"], state["skipped"],
               fmt(args.max_nodes) if args.max_nodes else "-", args.max_depth or "-"))
        if total_time > 0:
            out("total %s nodes in %.2fs (%s nps wall clock, includes UCI overhead)"
                % (fmt(total_nodes), total_time, fmt(int(total_nodes / total_time))))
        if args.json:
            with open(args.json, "w") as f:
                json.dump(state, f, indent=2)
        if log:
            log.close()

    if state["errors"]:
        return 2
    if state["failed"]:
        return 1
    if not state["passed"]:
        out("ERROR: nothing was tested (all entries skipped)")
        return 2
    return 0


def verify_suite(args, out=print):
    """Check suite counts within the limits against the builtin generator or a UCI reference."""
    suite = load_suite(args.suite)
    if args.only:
        wanted = set(args.only.split(","))
        suite = [e for e in suite if e["name"] in wanted]
    ref = None
    if args.reference not in ("builtin", "none"):
        ref = UciEngine(args.reference, args.timeout, "reference")
    bad = skipped = checked = 0
    try:
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
    return 1 if bad else 0


def self_test(args, out=print):
    """Run the harness against the mock engine, clean and with injected bugs."""
    mock = "%s -B %s" % (shlex.quote(sys.executable), shlex.quote(os.path.join(HERE, "mock_engine.py")))
    cases = (  # bug, --only, max nodes, expected exit, text that must appear
        ("", "", 60000, 0, "0 failed, 0 errors"),
        ("ep-pin", "illegal-ep-2", 60000, 1, "engine generates ILLEGAL move(s): c4d3"),
        ("no-underpromo", "cpw-pos4", 60000, 1,
         "engine MISSES legal move(s): b2a1b b2a1n b2a1r b2b1b b2b1n b2b1r"),
        ("castle-unmake", "kiwipete", 100000, 1, ">> state not restored"),
        ("ignore-moves", "startpos", 60000, 1, "probe  position startpos moves (en passant"),
        ("dup-moves", "startpos", 60000, 1, "engine lists move(s) more than once"),
        ("bad-total", "startpos", 60000, 2, "no total line (e.g. 'Nodes searched: N') recognised"),
    )
    ok = True
    for bug, only, max_nodes, want_rc, want_text in cases:
        out("== self-test: mock engine%s ==" % (" with bug " + bug if bug else ""))
        sub = make_parser().parse_args([])
        sub.engine = mock + (" --bug " + bug if bug else "")
        sub.only, sub.max_nodes, sub.timeout, sub.probe_timeout = only, max_nodes, 20, 3
        lines = []
        rc = run(sub, out=lambda s: (lines.append(s), out("  " + s)))
        good = rc == want_rc and want_text in "\n".join(lines)
        out("  -> %s (exit %d, expected %d; looked for %r)\n"
            % ("PASS" if good else "FAIL", rc, want_rc, want_text))
        ok &= good
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
                    help="max nodes the builtin reference will divide (default 5e6)")
    ap.add_argument("--timeout", type=float, default=600, help="seconds per go perft (default 600)")
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
    for name in ("max_nodes", "bisect_budget"):
        v = getattr(args, name)
        if v != v or v in (float("inf"), float("-inf")) or v < 0:
            ap.error("--%s must be a finite number >= 0" % name.replace("_", "-"))
        setattr(args, name, int(v))
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(2))  # run finally blocks, kill engines
    try:
        if args.verify_suite:
            return verify_suite(args)
        if args.self_test:
            return self_test(args)
        if not args.engine:
            ap.error("--engine is required")
        return run(args)
    except KeyboardInterrupt:
        print("interrupted")
        return 2
    except (OSError, ValueError, EngineError) as e:
        print("ERROR: %s" % e)
        return 2


if __name__ == "__main__":
    sys.exit(main())
