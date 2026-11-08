#!/usr/bin/env python3
"""Generate easime application icons (512, 256, 128, 64, 48, 32, 24, 16 px PNGs,
plus easime.ico and scalable easime.svg) in data/icons/.

Design concept (Multilingual Harmony Keycap):
- A modern dark rounded keycap with a subtle 3D bevel, glowing indigo/blue top
  rim, and an inner key face.
- Prominent Korean '한' on the main key face, accompanied by a crisp cyan 'A'
  badge (top-left) and a warm coral-amber 'あ' badge (bottom-right), connected
  by a subtle language-cycle arc representing effortless East Asian input.
"""

import math
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter, ImageFont

FONT_TTC = "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc"
INDEX_JP = 0
INDEX_KR = 1

OUT_DIR = Path(__file__).resolve().parent.parent / "data" / "icons"
SIZES = [512, 256, 128, 64, 48, 32, 24, 16]


def draw_centered_glyph(
    canvas: Image.Image,
    ch: str,
    font_idx: int,
    px_size: int,
    center_xy: tuple[float, float],
    color: tuple[int, int, int, int],
    shadow: bool = True,
):
    font = ImageFont.truetype(FONT_TTC, px_size, index=font_idx)
    bbox = font.getbbox(ch)
    w = bbox[2] - bbox[0]
    h = bbox[3] - bbox[1]
    cx, cy = center_xy
    x = cx - w / 2.0 - bbox[0]
    y = cy - h / 2.0 - bbox[1]

    if shadow:
        sh_layer = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
        sh_draw = ImageDraw.Draw(sh_layer)
        sh_draw.text(
            (x, y + canvas.width * 0.012), ch, font=font, fill=(6, 10, 22, 185)
        )
        sh_layer = sh_layer.filter(ImageFilter.GaussianBlur(radius=canvas.width * 0.012))
        canvas.alpha_composite(sh_layer)

    txt_layer = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    txt_draw = ImageDraw.Draw(txt_layer)
    txt_draw.text((x, y), ch, font=font, fill=color)
    canvas.alpha_composite(txt_layer)


def render_master(W: int = 1024) -> Image.Image:
    img = Image.new("RGBA", (W, W), (0, 0, 0, 0))

    # 1. Soft outer drop shadow under the keycap
    shadow = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    sdraw = ImageDraw.Draw(shadow)
    pad = int(W * 0.055)
    rad = int(W * 0.225)
    sdraw.rounded_rectangle(
        [pad, pad + int(W * 0.02), W - pad, W - pad + int(W * 0.025)],
        radius=rad,
        fill=(6, 10, 22, 190),
    )
    shadow = shadow.filter(ImageFilter.GaussianBlur(radius=W * 0.022))
    img.alpha_composite(shadow)

    # 2. Outer keycap base (3D lower skirt / bevel)
    base_mask = Image.new("L", (W, W), 0)
    bdraw = ImageDraw.Draw(base_mask)
    bdraw.rounded_rectangle([pad, pad, W - pad, W - pad], radius=rad, fill=255)

    base_grad = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    bg_px = base_grad.load()
    for y in range(W):
        t = max(0.0, min(1.0, (y - pad) / max(1, (W - 2 * pad))))
        r = int(40 * (1 - t) + 16 * t)
        g = int(50 * (1 - t) + 22 * t)
        b = int(78 * (1 - t) + 40 * t)
        for x in range(W):
            bg_px[x, y] = (r, g, b, 255)
    base_layer = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    base_layer.paste(base_grad, (0, 0), base_mask)

    ob_draw = ImageDraw.Draw(base_layer)
    ob_draw.rounded_rectangle(
        [pad, pad, W - pad, W - pad],
        radius=rad,
        outline=(98, 132, 200, 185),
        width=max(2, int(W * 0.006)),
    )
    img.alpha_composite(base_layer)

    # 3. Inner keycap top surface
    in_l = int(W * 0.095)
    in_t = int(W * 0.080)
    in_r = W - int(W * 0.095)
    in_b = W - int(W * 0.125)
    in_rad = int(W * 0.185)

    top_mask = Image.new("L", (W, W), 0)
    tdraw = ImageDraw.Draw(top_mask)
    tdraw.rounded_rectangle([in_l, in_t, in_r, in_b], radius=in_rad, fill=255)

    top_grad = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    tg_px = top_grad.load()
    for y in range(in_t, in_b + 1):
        ty = (y - in_t) / max(1, (in_b - in_t))
        for x in range(in_l, in_r + 1):
            tx = (x - in_l) / max(1, (in_r - in_l))
            dist = math.hypot(tx - 0.30, ty - 0.24)
            glow = max(0.0, 1.0 - dist * 1.15)
            r = int(28 + 24 * glow - 8 * ty)
            g = int(36 + 34 * glow - 10 * ty)
            b = int(58 + 56 * glow - 12 * ty)
            tg_px[x, y] = (
                max(0, min(255, r)),
                max(0, min(255, g)),
                max(0, min(255, b)),
                255,
            )

    top_layer = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    top_layer.paste(top_grad, (0, 0), top_mask)

    # Subtle language-cycle orbit arc on the keycap surface
    arc_layer = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    adraw = ImageDraw.Draw(arc_layer)
    arc_pad = int(W * 0.175)
    adraw.arc(
        [
            arc_pad,
            arc_pad - int(W * 0.015),
            W - arc_pad,
            W - arc_pad - int(W * 0.015),
        ],
        start=140,
        end=310,
        fill=(72, 185, 255, 68),
        width=int(W * 0.013),
    )
    adraw.arc(
        [
            arc_pad,
            arc_pad - int(W * 0.015),
            W - arc_pad,
            W - arc_pad - int(W * 0.015),
        ],
        start=320,
        end=130,
        fill=(255, 150, 110, 62),
        width=int(W * 0.013),
    )
    top_layer.alpha_composite(arc_layer)

    rim_draw = ImageDraw.Draw(top_layer)
    rim_draw.rounded_rectangle(
        [in_l, in_t, in_r, in_b],
        radius=in_rad,
        outline=(120, 165, 245, 115),
        width=max(2, int(W * 0.005)),
    )
    img.alpha_composite(top_layer)

    # 4. Main hero glyph '한' in the center of the keycap (drawn BEFORE corner
    #    badges so the corner badges sit cleanly in the foreground without
    #    colliding with the strokes of '한').
    draw_centered_glyph(
        img,
        "한",
        INDEX_KR,
        int(W * 0.415),
        (W * 0.49, W * 0.465),
        (255, 255, 255, 255),
        shadow=True,
    )

    # 5. Active composition underline bar at the bottom of the keycap face
    bar_w = int(W * 0.25)
    bar_h = int(W * 0.022)
    bar_x0 = int(W * 0.49 - bar_w / 2)
    bar_y0 = int(W * 0.755)
    bar_layer = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    bar_draw = ImageDraw.Draw(bar_layer)
    bar_draw.rounded_rectangle(
        [bar_x0, bar_y0, bar_x0 + bar_w, bar_y0 + bar_h],
        radius=bar_h // 2,
        fill=(88, 196, 255, 235),
    )
    img.alpha_composite(bar_layer)

    # 6. Top-left 'A' pill badge & Bottom-right 'あ' pill badge with drop shadows
    a_cx, a_cy = W * 0.235, W * 0.225
    a_r = int(W * 0.098)
    a_rad = int(W * 0.052)

    ja_cx, ja_cy = W * 0.765, W * 0.715
    ja_r = int(W * 0.104)
    ja_rad = int(W * 0.055)

    b_shadow = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    bs_draw = ImageDraw.Draw(b_shadow)
    bs_draw.rounded_rectangle(
        [
            int(a_cx - a_r),
            int(a_cy - a_r + W * 0.01),
            int(a_cx + a_r),
            int(a_cy + a_r + W * 0.01),
        ],
        radius=a_rad,
        fill=(6, 10, 22, 165),
    )
    bs_draw.rounded_rectangle(
        [
            int(ja_cx - ja_r),
            int(ja_cy - ja_r + W * 0.01),
            int(ja_cx + ja_r),
            int(ja_cy + ja_r + W * 0.01),
        ],
        radius=ja_rad,
        fill=(6, 10, 22, 165),
    )
    b_shadow = b_shadow.filter(ImageFilter.GaussianBlur(radius=W * 0.012))
    img.alpha_composite(b_shadow)

    badge_layer = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    b_draw = ImageDraw.Draw(badge_layer)
    b_draw.rounded_rectangle(
        [int(a_cx - a_r), int(a_cy - a_r), int(a_cx + a_r), int(a_cy + a_r)],
        radius=a_rad,
        fill=(22, 92, 168, 248),
        outline=(115, 212, 255, 240),
        width=max(2, int(W * 0.006)),
    )
    b_draw.rounded_rectangle(
        [int(ja_cx - ja_r), int(ja_cy - ja_r), int(ja_cx + ja_r), int(ja_cy + ja_r)],
        radius=ja_rad,
        fill=(168, 56, 72, 248),
        outline=(255, 172, 148, 240),
        width=max(2, int(W * 0.006)),
    )
    img.alpha_composite(badge_layer)

    draw_centered_glyph(
        img,
        "A",
        INDEX_KR,
        int(W * 0.136),
        (a_cx, a_cy),
        (244, 252, 255, 255),
        shadow=False,
    )
    draw_centered_glyph(
        img,
        "あ",
        INDEX_JP,
        int(W * 0.134),
        (ja_cx, ja_cy),
        (255, 246, 242, 255),
        shadow=False,
    )

    return img


def write_svg(path: Path):
    svg = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <defs>
    <linearGradient id="bevel" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#28324e"/>
      <stop offset="100%" stop-color="#101628"/>
    </linearGradient>
    <radialGradient id="face" cx="30%" cy="24%" r="75%">
      <stop offset="0%" stop-color="#344672"/>
      <stop offset="100%" stop-color="#161e32"/>
    </radialGradient>
    <filter id="shadow" x="-10%" y="-10%" width="120%" height="125%">
      <feDropShadow dx="0" dy="8" stdDeviation="10" flood-color="#060a16" flood-opacity="0.72"/>
    </filter>
  </defs>
  <!-- Outer keycap bevel -->
  <rect x="28" y="28" width="456" height="456" rx="115" fill="url(#bevel)" stroke="#6284c8" stroke-width="3" stroke-opacity="0.72" filter="url(#shadow)"/>
  <!-- Inner keycap top surface -->
  <rect x="49" y="41" width="414" height="406" rx="95" fill="url(#face)" stroke="#78a5f5" stroke-width="2.5" stroke-opacity="0.45"/>
  <!-- Language cycle orbit -->
  <circle cx="256" cy="248" r="166" fill="none" stroke="#48b9ff" stroke-width="6.5" stroke-opacity="0.25" stroke-dasharray="220 120"/>
  <!-- Main Korean '한' glyph -->
  <text x="251" y="314" text-anchor="middle" font-family="'Noto Sans CJK KR', 'Noto Sans KR', sans-serif" font-weight="700" font-size="212" fill="#ffffff">한</text>
  <!-- Composition underline accent -->
  <rect x="187" y="386" width="128" height="11" rx="5.5" fill="#58c4ff"/>
  <!-- English 'A' badge (top-left) -->
  <rect x="70" y="65" width="100" height="100" rx="26" fill="#165ca8" fill-opacity="0.97" stroke="#73d4ff" stroke-width="3"/>
  <text x="120" y="138" text-anchor="middle" font-family="'Noto Sans CJK KR', 'Noto Sans', sans-serif" font-weight="700" font-size="68" fill="#f4fcff">A</text>
  <!-- Japanese 'あ' badge (bottom-right) -->
  <rect x="339" y="313" width="106" height="106" rx="28" fill="#a83848" fill-opacity="0.97" stroke="#ffac94" stroke-width="3"/>
  <text x="392" y="389" text-anchor="middle" font-family="'Noto Sans CJK JP', 'Noto Sans JP', sans-serif" font-weight="700" font-size="66" fill="#fff6f2">あ</text>
</svg>
"""
    path.write_text(svg, encoding="utf-8")


def main():
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    master = render_master(1024)

    for s in SIZES:
        resized = master.resize((s, s), Image.Resampling.LANCZOS)
        out_path = OUT_DIR / f"easime-{s}.png"
        resized.save(out_path, format="PNG", optimize=True)
        print(f"wrote {out_path} ({s}x{s})")

    ico_sizes = [
        (16, 16),
        (24, 24),
        (32, 32),
        (48, 48),
        (64, 64),
        (128, 128),
        (256, 256),
    ]
    ico_img = master.resize((256, 256), Image.Resampling.LANCZOS)
    ico_path = OUT_DIR / "easime.ico"
    ico_img.save(ico_path, format="ICO", sizes=ico_sizes)
    print(f"wrote {ico_path}")

    svg_path = OUT_DIR / "easime.svg"
    write_svg(svg_path)
    print(f"wrote {svg_path}")


if __name__ == "__main__":
    main()
