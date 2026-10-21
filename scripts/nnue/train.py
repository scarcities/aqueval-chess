#!/usr/bin/env python3
"""Quant's own NNUE trainer: (768 -> 256)x2 -> 1, SCReLU, CPU or CUDA via PyTorch.

Writes nets in exactly the layout the engine loads (src/nnue, docs/NNUE.md), which is the
same layout bullet's quantised.bin has with the SavedFormat listed in docs/NNUE.md, so nets
from either trainer are interchangeable.

Subcommands:
  prepare  FILES... --out DIR      text "<FEN> | <cp> | <result>" (White's view) -> packed arrays
  train    --data DIR --out DIR    train, writing a checkpoint and a quantised net per epoch
  export   --ckpt FILE --out NET   checkpoint -> quantised net
  eval     --net NET --fen FEN     exact integer eval, as the engine computes it (cross-check)

Needs numpy; train/export also need torch (CPU build: pip install torch --index-url
https://download.pytorch.org/whl/cpu).
"""

import argparse
import math
import os
import sys
import time
from multiprocessing import Pool

import numpy as np

INPUTS = 768
HIDDEN = 256
QA, QB, SCALE = 255, 64, 400
MAX_PIECES = 32
PAD = 0xFFFF
PIECES = "PNBRQKpnbrqk"

# ---------------------------------------------------------------- data preparation
#
# Packed format (one .npy per field, row i = position i):
#   pieces  uint16 (N, 32)  White-view feature c*384 + type*64 + square (c: 0 White piece,
#                           1 Black piece; square a1 = 0), padded with 0xFFFF
#   stm     uint8  (N,)     0 White to move, 1 Black to move
#   score   int16  (N,)     centipawns, White's view
#   result  uint8  (N,)     0 Black won, 1 draw, 2 White won


def parse_line(line):
    fen, score, result = (p.strip() for p in line.split("|"))
    fields = fen.split()
    feats = []
    rank, file = 7, 0
    for ch in fields[0]:
        if ch == "/":
            rank, file = rank - 1, 0
        elif ch.isdigit():
            file += int(ch)
        else:
            p = PIECES.index(ch)
            feats.append((p // 6) * 384 + (p % 6) * 64 + rank * 8 + file)
            file += 1
    stm = 0 if fields[1] == "w" else 1
    return feats, stm, int(score), {"0.0": 0, "0.5": 1, "1.0": 2}[result]


def count_lines(path):
    with open(path, "rb") as f:
        return sum(1 for _ in f)


def pack_file(job):
    path, out_dir, start, n = job
    pieces = np.lib.format.open_memmap(os.path.join(out_dir, "pieces.npy"), mode="r+")
    stm = np.lib.format.open_memmap(os.path.join(out_dir, "stm.npy"), mode="r+")
    score = np.lib.format.open_memmap(os.path.join(out_dir, "score.npy"), mode="r+")
    result = np.lib.format.open_memmap(os.path.join(out_dir, "result.npy"), mode="r+")
    with open(path) as f:
        for i, line in enumerate(f):
            if i >= n:
                break
            feats, s, cp, r = parse_line(line)
            row = start + i
            pieces[row, : len(feats)] = feats
            stm[row], score[row], result[row] = s, max(-32767, min(32767, cp)), r
    pieces.flush()
    return n


def cmd_prepare(args):
    os.makedirs(args.out, exist_ok=True)
    with Pool(args.threads) as pool:
        counts = pool.map(count_lines, args.files)
    total = sum(counts)
    print(f"{total} positions in {len(args.files)} files")
    np.lib.format.open_memmap(os.path.join(args.out, "pieces.npy"), mode="w+", dtype=np.uint16, shape=(total, MAX_PIECES))[:] = PAD
    for name, dt in (("stm", np.uint8), ("score", np.int16), ("result", np.uint8)):
        np.lib.format.open_memmap(os.path.join(args.out, f"{name}.npy"), mode="w+", dtype=dt, shape=(total,))
    jobs, start = [], 0
    for path, n in zip(args.files, counts):
        jobs.append((path, args.out, start, n))
        start += n
    t = time.time()
    with Pool(args.threads) as pool:
        for done in pool.imap_unordered(pack_file, jobs):
            print(f"  packed {done} positions ({time.time() - t:.0f} s)")
    print(f"wrote {args.out}")


# ---------------------------------------------------------------- model


def perspective_features(pieces, stm):
    """(B, 32) White-view features + side to move -> (us, them) index tensors, 768 = padding."""
    import torch

    valid = pieces != PAD
    p = pieces.long().clamp(max=INPUTS - 1)
    colour, rest = p // 384, p % 384
    white_view = p
    black_view = (1 - colour) * 384 + (rest // 64) * 64 + ((rest % 64) ^ 56)
    white_view = torch.where(valid, white_view, INPUTS)
    black_view = torch.where(valid, black_view, INPUTS)
    black_to_move = stm.bool().unsqueeze(1)
    us = torch.where(black_to_move, black_view, white_view)
    them = torch.where(black_to_move, white_view, black_view)
    return us, them


def make_model(torch):
    nn = torch.nn

    class Net(nn.Module):
        def __init__(self):
            super().__init__()
            # Row INPUTS is padding (always zero, never trained, not exported).
            self.ft = nn.EmbeddingBag(INPUTS + 1, HIDDEN, mode="sum", padding_idx=INPUTS)
            self.ft_bias = nn.Parameter(torch.zeros(HIDDEN))
            self.out = nn.Linear(2 * HIDDEN, 1)
            with torch.no_grad():
                self.ft.weight.normal_(0, 0.05)
                self.ft.weight[INPUTS].zero_()
                self.ft_bias.uniform_(0, 0.1)
                self.out.weight.normal_(0, 1 / math.sqrt(2 * HIDDEN))
                self.out.bias.zero_()

        def forward(self, us, them):
            a = (self.ft(us) + self.ft_bias).clamp(0, 1).square()
            b = (self.ft(them) + self.ft_bias).clamp(0, 1).square()
            return self.out(torch.cat([a, b], dim=1)).squeeze(1)  # eval / SCALE

        def clip(self, limit=1.98):
            # Keeps quantised values inside i16, and |output weight| * QB <= 127 for the
            # engine's AVX2 path.
            with torch.no_grad():
                self.ft.weight.clamp_(-limit, limit)
                self.out.weight.clamp_(-limit, limit)

    return Net()


def export_net(model, path):
    w = model.ft.weight.detach().cpu().numpy()[:INPUTS]  # (768, H): feature-major
    parts = [
        np.round(w * QA),
        np.round(model.ft_bias.detach().cpu().numpy() * QA),
        np.round(model.out.weight.detach().cpu().numpy().reshape(-1) * QB),  # us, then them
        np.round(model.out.bias.detach().cpu().numpy() * QA * QB),
    ]
    flat = np.concatenate([p.reshape(-1) for p in parts])
    if np.abs(flat).max() > 32767:
        raise ValueError("weights overflow i16; clip harder")
    data = flat.astype("<i2").tobytes()
    data += b"\0" * (-len(data) % 64)
    with open(path, "wb") as f:
        f.write(data)


# ---------------------------------------------------------------- training


def cmd_train(args):
    import torch

    torch.manual_seed(args.seed)
    if args.threads:
        torch.set_num_threads(args.threads)
    device = torch.device("cuda" if torch.cuda.is_available() and not args.cpu else "cpu")
    load = lambda name: np.load(os.path.join(args.data, f"{name}.npy"), mmap_mode=None if args.in_memory else "r")
    pieces, stm, score, result = (load(n) for n in ("pieces", "stm", "score", "result"))
    n = len(stm)
    rng = np.random.default_rng(args.seed)
    order = rng.permutation(n)
    n_val = min(n // 200, 200_000)
    val_idx, train_idx = np.sort(order[:n_val]), order[n_val:]
    print(f"{n} positions ({len(train_idx)} train, {n_val} validation) on {device}")

    model = make_model(torch).to(device)
    opt = torch.optim.AdamW(model.parameters(), lr=args.lr, weight_decay=args.weight_decay)
    start_epoch = 1
    if args.resume:
        ckpt = torch.load(args.resume, map_location=device)
        model.load_state_dict(ckpt["model"])
        opt.load_state_dict(ckpt["opt"])
        start_epoch = ckpt["epoch"] + 1

    def batch(idx):
        idx = np.sort(idx)  # sequential reads when memory-mapped
        p = torch.from_numpy(pieces[idx].astype(np.int32)).to(device)
        s = torch.from_numpy(stm[idx].astype(np.int64)).to(device)
        cp = torch.from_numpy(score[idx].astype(np.float32)).to(device)
        res = torch.from_numpy(result[idx].astype(np.float32) / 2).to(device)
        # Targets from the side to move's view.
        sign = 1 - 2 * s.float()
        cp, res = cp * sign, torch.where(s.bool(), 1 - res, res)
        target = args.wdl * res + (1 - args.wdl) * torch.sigmoid(cp / SCALE)
        us, them = perspective_features(p, s)
        return us, them, target

    def loss_of(us, them, target):
        return (torch.sigmoid(model(us, them)) - target).square().mean()

    os.makedirs(args.out, exist_ok=True)
    steps = math.ceil(len(train_idx) / args.batch)
    for epoch in range(start_epoch, args.epochs + 1):
        # Cosine decay from lr to lr * final_lr over all epochs.
        frac = (epoch - 1) / max(1, args.epochs - 1)
        lr = args.lr * (args.final_lr + (1 - args.final_lr) * 0.5 * (1 + math.cos(math.pi * frac)))
        for g in opt.param_groups:
            g["lr"] = lr
        perm = rng.permutation(train_idx)
        model.train()
        t, total = time.time(), 0.0
        for k in range(steps):
            us, them, target = batch(perm[k * args.batch : (k + 1) * args.batch])
            loss = loss_of(us, them, target)
            opt.zero_grad(set_to_none=True)
            loss.backward()
            opt.step()
            model.clip()
            total += loss.item()
            if (k + 1) % 200 == 0:
                print(f"  epoch {epoch} step {k + 1}/{steps} loss {total / (k + 1):.6f} ({(k + 1) * args.batch / (time.time() - t):.0f} pos/s)", flush=True)
        model.eval()
        with torch.no_grad():
            val = np.mean([loss_of(*batch(val_idx[i : i + args.batch])).item() for i in range(0, n_val, args.batch)]) if n_val else float("nan")
        name = os.path.join(args.out, f"epoch{epoch:03d}")
        torch.save({"model": model.state_dict(), "opt": opt.state_dict(), "epoch": epoch}, name + ".pt")
        export_net(model, name + ".nnue")
        print(f"epoch {epoch}/{args.epochs} lr {lr:.2e} train {total / steps:.6f} val {val:.6f} ({time.time() - t:.0f} s) -> {name}.nnue", flush=True)


def cmd_export(args):
    import torch

    model = make_model(torch)
    model.load_state_dict(torch.load(args.ckpt, map_location="cpu")["model"])
    export_net(model, args.out)
    print(f"wrote {args.out}")


# ---------------------------------------------------------------- exact integer eval


def tdiv(a, b):
    """Integer division truncating towards zero, like Rust's `/`."""
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b > 0) else -q


def cmd_eval(args):
    raw = np.fromfile(args.net, dtype="<i2").astype(np.int64)
    ft_w = raw[: INPUTS * HIDDEN].reshape(INPUTS, HIDDEN)
    ft_b = raw[INPUTS * HIDDEN : INPUTS * HIDDEN + HIDDEN]
    o = INPUTS * HIDDEN + HIDDEN
    out_w, out_b = raw[o : o + 2 * HIDDEN].reshape(2, HIDDEN), int(raw[o + 2 * HIDDEN])
    feats, stm, _, _ = parse_line(f"{args.fen} | 0 | 0.5")
    p = np.array(feats)
    colour, rest = p // 384, p % 384
    views = [p, (1 - colour) * 384 + (rest // 64) * 64 + ((rest % 64) ^ 56)]  # White, Black
    us, them = views[stm], views[1 - stm]
    total = 0
    for k, idx in enumerate((us, them)):
        acc = (ft_b + ft_w[idx].sum(axis=0)).astype(np.int16).astype(np.int64)  # i16 wraparound
        c = np.clip(acc, 0, QA)
        total += int((c * c * out_w[k]).sum())
    out = tdiv(total, QA) + out_b
    cp = max(-20000, min(20000, tdiv(out * SCALE, QA * QB)))
    print(f"NNUE eval: {cp} cp (side to move)")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("prepare")
    p.add_argument("files", nargs="+")
    p.add_argument("--out", required=True)
    p.add_argument("--threads", type=int, default=os.cpu_count())
    p = sub.add_parser("train")
    p.add_argument("--data", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--epochs", type=int, default=40)
    p.add_argument("--batch", type=int, default=16384)
    p.add_argument("--lr", type=float, default=1e-3)
    p.add_argument("--final-lr", type=float, default=0.01, help="final lr as a fraction of --lr")
    p.add_argument("--wdl", type=float, default=0.25, help="weight of the game result in the target")
    p.add_argument("--weight-decay", type=float, default=0.0)
    p.add_argument("--threads", type=int, default=0)
    p.add_argument("--seed", type=int, default=1)
    p.add_argument("--cpu", action="store_true", help="ignore CUDA")
    p.add_argument("--in-memory", action="store_true", help="load the data into RAM (faster)")
    p.add_argument("--resume")
    p = sub.add_parser("export")
    p.add_argument("--ckpt", required=True)
    p.add_argument("--out", required=True)
    p = sub.add_parser("eval")
    p.add_argument("--net", required=True)
    p.add_argument("--fen", required=True)
    args = ap.parse_args()
    {"prepare": cmd_prepare, "train": cmd_train, "export": cmd_export, "eval": cmd_eval}[args.cmd](args)


if __name__ == "__main__":
    sys.exit(main())
