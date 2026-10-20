# 生成应用图标源图 app-icon.png（1024x1024）。
# 设计与 web/favicon.svg、旧版 tray.icon_image 一致：蓝色渐变圆角方块 + 两道白条 + 绿点。
# 用法：python scripts/gen_icon.py  （生成后用 `pnpm tauri icon app-icon.png` 出全套图标）

from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 1024


def icon_image(size: int = SIZE) -> Image.Image:
    scale = size / 64

    def box(x0: float, y0: float, x1: float, y1: float) -> list[int]:
        return [round(v * scale) for v in (x0, y0, x1, y1)]

    # 渐变底
    base = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    top, bottom = (59, 130, 246), (29, 78, 216)
    draw = ImageDraw.Draw(base)
    for y in range(size):
        t = y / max(size - 1, 1)
        color = tuple(round(top[i] + (bottom[i] - top[i]) * t) for i in range(3)) + (255,)
        draw.line([(0, y), (size, y)], fill=color)

    # 圆角遮罩
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        box(1, 1, 63, 63), radius=round(15 * scale), fill=255
    )
    icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    icon.paste(base, (0, 0), mask)

    overlay = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(overlay)
    d.rounded_rectangle(box(11, 19, 53, 31), radius=round(6 * scale), fill=(255, 255, 255, 245))
    d.rounded_rectangle(box(11, 34, 53, 46), radius=round(6 * scale), fill=(255, 255, 255, 140))
    cx, cy = round(46 * scale), round(25 * scale)
    halo = round(9 * scale)
    d.ellipse([cx - halo, cy - halo, cx + halo, cy + halo], fill=(74, 222, 128, 97))
    r = max(2, round(4.6 * scale))
    d.ellipse(
        [cx - r, cy - r, cx + r, cy + r],
        fill=(34, 197, 94, 255),
        outline=(255, 255, 255, 255),
        width=max(1, round(2.2 * scale)),
    )
    return Image.alpha_composite(icon, overlay)


def main() -> None:
    out = Path(__file__).resolve().parents[1] / "app-icon.png"
    icon_image().save(out, "PNG")
    print(f"written: {out}")


if __name__ == "__main__":
    main()
