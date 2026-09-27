"""Widescreen extension art for Reunion pictures, via fal.ai FLUX.

  widen.py generate GRAFIKA/MAIN.PIC [--backend lama|outpaint|fill] [--count N] [--prompt TEXT]
  widen.py process  GRAFIKA/MAIN.PIC ATTEMPT [--dither]
  widen.py sheet    GRAFIKA/MAIN.PIC

generate: scales the picture up 4x (sharp pixels), has FLUX paint EXPAND
          columns (at 1x) on both sides and keeps every full high-res result in
          art-src/widescreen/<DIR>/<NAME>/attempts/ with a .json of its settings.
process:  turns one attempt into the strips the engine loads: scale to 1x,
          reduce to the picture's own palette, put the original back in the
          middle untouched, cut into <NAME>.left.png / <NAME>.right.png under
          crates/reunion/art/widescreen/<DIR>/.
sheet:    previews every attempt (processed, 16:10 and 16:9 crops) for picking.

Backends: `lama` runs LaMa locally (free, placeholder quality; the model is
downloaded to tools/models/ on first use). `outpaint` and `fill` use FLUX on
fal.ai and need FAL_KEY in the environment.
"""
import argparse
import base64
import io
import json
import os
import sys
import time
import urllib.request
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from pic2png import decode_pic  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
GAME = ROOT / "game"
SRC = ROOT / "art-src" / "widescreen"
OUT = ROOT / "crates" / "reunion" / "art" / "widescreen"

SCALE = 4
# 16:9 needs 427 columns: 54 more on each side. 16:10 shows 32 of them.
EXPAND = 54

LAMA_MODEL = ROOT / "tools" / "models" / "lama_fp32.onnx"
LAMA_URL = "https://huggingface.co/Carve/LaMa-ONNX/resolve/main/lama_fp32.onnx"
# LaMa takes a fixed 512x512 window; at 3x the tallest pictures (151 rows) fit.
LAMA_SCALE = 3
LAMA_WINDOW = 512

MODELS = {
    "lama": "LaMa (Carve/LaMa-ONNX lama_fp32)",
    "outpaint": "fal-ai/flux-2-pro/outpaint",
    "fill": "fal-ai/flux-pro/v1/fill",
}
DEFAULT_PROMPT = (
    "Continue the scene seamlessly to the left and right, same perspective, lighting "
    "and 1990s VGA pixel art style. Extend only walls, floor and machinery; "
    "no people, no text, no new doors or screens."
)


def paths(pic: str):
    rel = Path(pic)
    name = rel.stem.upper()
    return SRC / rel.parent / name, OUT / rel.parent, name


def load(pic: str) -> Image.Image:
    return decode_pic((GAME / pic).read_bytes())


def data_uri(img: Image.Image) -> str:
    buf = io.BytesIO()
    img.save(buf, "PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


def fal(model: str, payload: dict) -> Image.Image:
    key = os.environ.get("FAL_KEY")
    if not key:
        sys.exit("FAL_KEY is not set")
    req = urllib.request.Request(
        f"https://fal.run/{model}",
        data=json.dumps(payload).encode(),
        headers={"Authorization": f"Key {key}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=600) as resp:
        result = json.load(resp)
    url = result["images"][0]["url"]
    if url.startswith("data:"):
        raw = base64.b64decode(url.split(",", 1)[1])
    else:
        with urllib.request.urlopen(url, timeout=600) as resp:
            raw = resp.read()
    return Image.open(io.BytesIO(raw)).convert("RGB"), result


def lama(orig: Image.Image) -> Image.Image:
    """Extends both sides with LaMa, one 512x512 window per side."""
    import onnxruntime as ort

    if not LAMA_MODEL.exists():
        LAMA_MODEL.parent.mkdir(parents=True, exist_ok=True)
        print("downloading LaMa model (200 MB)...")
        urllib.request.urlretrieve(LAMA_URL, LAMA_MODEL)
    session = ort.InferenceSession(str(LAMA_MODEL))
    big = np.array(orig.resize((orig.width * LAMA_SCALE, orig.height * LAMA_SCALE), Image.NEAREST)).astype(np.float32) / 255
    h = big.shape[0]
    ext = EXPAND * LAMA_SCALE
    context = LAMA_WINDOW - ext
    if h > LAMA_WINDOW:
        sys.exit(f"picture too tall for a {LAMA_WINDOW} window at {LAMA_SCALE}x")

    def extend_left(img):
        window = np.zeros((LAMA_WINDOW, LAMA_WINDOW, 3), np.float32)
        mask = np.zeros((LAMA_WINDOW, LAMA_WINDOW), np.float32)
        window[:h, ext:] = img[:, :context]
        # Mirror rows below the picture as context; they're cropped off again.
        window[h:, ext:] = img[h - 1 - np.arange(LAMA_WINDOW - h) % h, :context]
        mask[:, :ext] = 1
        out = session.run(None, {"image": window.transpose(2, 0, 1)[None], "mask": mask[None, None]})[0][0]
        out = out.transpose(1, 2, 0)
        scale = 255.0 if out.max() <= 1.5 else 1.0
        return np.clip(out[:h, :ext] * scale, 0, 255).astype(np.uint8)

    left = extend_left(big)
    right = extend_left(big[:, ::-1])[:, ::-1]
    return Image.fromarray(np.concatenate([left, (big * 255).astype(np.uint8), right], 1))


def generate(args):
    src_dir, _, name = paths(args.pic)
    attempts = src_dir / "attempts"
    attempts.mkdir(parents=True, exist_ok=True)
    orig = load(args.pic).convert("RGB")
    w, h = orig.size
    big = orig.resize((w * SCALE, h * SCALE), Image.NEAREST)
    pad = EXPAND * SCALE
    target = ((w + 2 * EXPAND) * SCALE, h * SCALE)

    for n in range(args.count):
        started = time.time()
        if args.backend == "lama":
            # Deterministic: one attempt is all there is to get.
            img, result, payload = lama(orig), {}, {}
            target = img.size
        elif args.backend == "outpaint":
            payload = {
                "image_url": data_uri(big),
                "expand_left": pad,
                "expand_right": pad,
                "output_format": "png",
                "sync_mode": True,
            }
        else:
            canvas = Image.new("RGB", target)
            canvas.paste(big, (pad, 0))
            mask = Image.new("L", target, 255)
            mask.paste(0, (pad, 0, pad + big.width, big.height))
            payload = {
                "prompt": args.prompt,
                "image_url": data_uri(canvas),
                "mask_url": data_uri(mask),
                "output_format": "png",
                "sync_mode": True,
            }
        if args.backend != "lama":
            img, result = fal(MODELS[args.backend], payload)
        if img.size != target:
            print(f"  model returned {img.size}, resizing to {target}")
            img = img.resize(target, Image.LANCZOS)
        stamp = time.strftime("%Y%m%d-%H%M%S")
        stem = f"{stamp}-{args.backend}-{n}"
        img.save(attempts / f"{stem}.png")
        meta = {
            "picture": args.pic,
            "model": MODELS[args.backend],
            "scale": LAMA_SCALE if args.backend == "lama" else SCALE,
            "expand": EXPAND,
            "prompt": payload.get("prompt"),
            "seed": result.get("seed"),
            "seconds": round(time.time() - started, 1),
        }
        (attempts / f"{stem}.json").write_text(json.dumps(meta, indent=2))
        print(f"{stem}.png ({meta['seconds']}s)")


def to_1x(pic: str, attempt: Path, dither: bool) -> Image.Image:
    """High-res attempt -> 1x picture in the original's palette, original pixels restored."""
    orig = load(pic)
    w, h = orig.size
    wide = Image.open(attempt).convert("RGB").resize((w + 2 * EXPAND, h), Image.BOX)

    # Palette of only the colors the original uses, so nothing new appears.
    idx = np.array(orig)
    pal = np.array(orig.getpalette()[:768]).reshape(256, 3)
    used = np.unique(idx)
    palimg = Image.new("P", (1, 1))
    flat = [int(v) for i in range(256) for v in pal[used[min(i, len(used) - 1)]]]
    palimg.putpalette(flat)
    q = np.array(wide.quantize(palette=palimg, dither=Image.Dither.FLOYDSTEINBERG if dither else Image.Dither.NONE))
    out = used[np.minimum(q, len(used) - 1)]
    out[:, EXPAND:EXPAND + w] = idx
    img = Image.fromarray(out.astype(np.uint8), "P")
    img.putpalette(orig.getpalette())
    return img


def seam_report(img: Image.Image, w: int) -> str:
    rgb = np.array(img.convert("RGB")).astype(int)
    col = lambda x: rgb[:, x]
    inner = np.mean([np.abs(col(x) - col(x + 1)).mean() for x in range(EXPAND, EXPAND + w - 1)])
    left = np.abs(col(EXPAND - 1) - col(EXPAND)).mean()
    right = np.abs(col(EXPAND + w - 1) - col(EXPAND + w)).mean()
    return f"seam jump left {left / inner:.2f}x, right {right / inner:.2f}x the original's column-to-column change"


def preview(img: Image.Image, w: int) -> Image.Image:
    h = img.height
    crops = [img.crop((EXPAND - 32, 0, EXPAND + w + 32, h)), img]
    rows = [c.convert("RGB").resize((c.width * 2, int(h * 1.2 * 2)), Image.NEAREST) for c in crops]
    sheet = Image.new("RGB", (rows[1].width, rows[0].height + rows[1].height + 8), (90, 90, 90))
    sheet.paste(rows[0], ((rows[1].width - rows[0].width) // 2, 0))
    sheet.paste(rows[1], (0, rows[0].height + 8))
    return sheet


def process(args):
    src_dir, out_dir, name = paths(args.pic)
    attempt = src_dir / "attempts" / args.attempt
    if not attempt.suffix:
        attempt = attempt.with_suffix(".png")
    img = to_1x(args.pic, attempt, args.dither)
    w = img.width - 2 * EXPAND
    out_dir.mkdir(parents=True, exist_ok=True)
    img.crop((0, 0, EXPAND, img.height)).convert("RGBA").save(out_dir / f"{name}.left.png")
    img.crop((EXPAND + w, 0, img.width, img.height)).convert("RGBA").save(out_dir / f"{name}.right.png")
    (src_dir / "chosen.txt").write_text(f"{attempt.name}{' dither' if args.dither else ''}\n")
    preview(img, w).save(src_dir / "preview.png")
    print(f"wrote {name}.left.png / {name}.right.png from {attempt.name}")
    print(seam_report(img, w))


def sheet(args):
    src_dir, _, name = paths(args.pic)
    attempts = sorted((src_dir / "attempts").glob("*.png"))
    if not attempts:
        sys.exit("no attempts yet")
    previews = []
    for a in attempts:
        img = to_1x(args.pic, a, dither=False)
        previews.append((a.name, preview(img, img.width - 2 * EXPAND)))
        print(f"{a.name}: {seam_report(img, img.width - 2 * EXPAND)}")
    width = max(p.width for _, p in previews)
    total = sum(p.height + 20 for _, p in previews)
    out = Image.new("RGB", (width, total), (40, 40, 40))
    y = 0
    for label, p in previews:
        from PIL import ImageDraw
        ImageDraw.Draw(out).text((4, y + 2), label, fill=(255, 255, 255))
        out.paste(p, (0, y + 18))
        y += p.height + 20
    out.save(src_dir / "sheet.png")
    print(f"sheet: {src_dir / 'sheet.png'}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    g = sub.add_parser("generate")
    g.add_argument("pic")
    g.add_argument("--backend", choices=MODELS, default="lama")
    g.add_argument("--count", type=int, default=2)
    g.add_argument("--prompt", default=DEFAULT_PROMPT)
    p = sub.add_parser("process")
    p.add_argument("pic")
    p.add_argument("attempt")
    p.add_argument("--dither", action="store_true")
    s = sub.add_parser("sheet")
    s.add_argument("pic")
    args = parser.parse_args()
    {"generate": generate, "process": process, "sheet": sheet}[args.command](args)


if __name__ == "__main__":
    main()
