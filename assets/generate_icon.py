#!/usr/bin/env python3
"""
FastPaste Icon V2 - Improved contrast and scale for tray visibility
"""
from PIL import Image, ImageDraw
import os

COLORS = {
    "bg": (0, 0, 0, 0),
    "clipboard_bg": (255, 255, 255, 255),
    "clipboard_border": (107, 114, 128, 255),  # gray-500 stronger
    "clipboard_border_light": (156, 163, 175, 255),
    "clip_top": (17, 24, 39, 255),  # gray-900 very dark
    "clip_highlight": (75, 85, 99, 255),  # gray-600
    "line": (229, 231, 235, 255),  # gray-200
    "line_mid": (156, 163, 175, 255),  # gray-400
    "line_light": (209, 213, 219, 255),  # gray-300
    "accent": (37, 99, 235, 255),  # blue-600
    "accent_dark": (29, 78, 216, 255),
    "accent_light": (96, 165, 250, 255),
    "lightning": (255, 255, 255, 255),
    "shadow": (0, 0, 0, 25),
    "shadow2": (0, 0, 0, 40),
}

def rounded_rect(draw, xy, radius, fill, outline=None, width=1):
    try:
        draw.rounded_rectangle(xy, radius=radius, fill=fill, outline=outline, width=width)
    except:
        x0,y0,x1,y1 = xy
        draw.rectangle([x0+radius, y0, x1-radius, y1], fill=fill, outline=outline, width=width)
        draw.rectangle([x0, y0+radius, x1, y1-radius], fill=fill, outline=outline, width=width)
        draw.ellipse([x0, y0, x0+radius*2, y0+radius*2], fill=fill, outline=outline, width=width)
        draw.ellipse([x1-radius*2, y0, x1, y0+radius*2], fill=fill, outline=outline, width=width)
        draw.ellipse([x0, y1-radius*2, x0+radius*2, y1], fill=fill, outline=outline, width=width)
        draw.ellipse([x1-radius*2, y1-radius*2, x1, y1], fill=fill, outline=outline, width=width)

def draw_icon(size, output_path=None):
    img = Image.new("RGBA", (size, size), COLORS["bg"])
    draw = ImageDraw.Draw(img)
    scale = size / 256.0

    # For small sizes, use larger relative dimensions to fill canvas
    if size <= 24:
        # Use 85% fill for tiny icons
        bw, bh = 0.72, 0.78
        bx0 = (1 - bw)/2
        by0 = 0.16
    elif size <= 32:
        bw, bh = 0.68, 0.72
        bx0 = (1 - bw)/2
        by0 = 0.18
    elif size <= 48:
        bw, bh = 0.62, 0.70
        bx0 = (1 - bw)/2
        by0 = 0.19
    else:
        bw, bh = 0.58, 0.68
        bx0 = (1 - bw)/2
        by0 = 0.20

    body_x0 = int(bx0 * 256 * scale)
    body_y0 = int(by0 * 256 * scale)
    body_x1 = int((bx0 + bw) * 256 * scale)
    body_y1 = int((by0 + bh) * 256 * scale)

    # Shadow
    if size >= 24:
        shadow_off = max(1, int(3*scale)) if size >= 48 else max(1, int(2*scale))
        sr = int(12*scale) if size >= 32 else int(6*scale)
        try:
            draw.rounded_rectangle([body_x0+shadow_off, body_y0+shadow_off, body_x1+shadow_off, body_y1+shadow_off],
                                   radius=sr, fill=COLORS["shadow"])
        except:
            pass
        if size >= 48:
            # double shadow for depth
            try:
                draw.rounded_rectangle([body_x0+1, body_y0+1, body_x1+1, body_y1+1],
                                       radius=sr, fill=(0,0,0,15))
            except:
                pass

    body_radius = int(16*scale) if size >= 64 else int(12*scale) if size >= 32 else int(8*scale) if size >= 24 else int(5*scale)
    border_w = max(1, int(2.5*scale)) if size >= 48 else max(1, int(2*scale)) if size >= 24 else 1

    # Clipboard body with stronger border
    # For small sizes, use a slightly darker border to be visible on light bg
    border_col = COLORS["clipboard_border"] if size >= 32 else (75, 85, 99, 255)
    rounded_rect(draw, [body_x0, body_y0, body_x1, body_y1],
                 radius=body_radius, fill=COLORS["clipboard_bg"],
                 outline=border_col, width=border_w)

    # Clip - proportionally larger for small sizes
    if size <= 24:
        clip_w_frac, clip_h_frac = 0.42, 0.13
    elif size <= 32:
        clip_w_frac, clip_h_frac = 0.38, 0.12
    else:
        clip_w_frac, clip_h_frac = 0.32, 0.11

    clip_w = int(256 * clip_w_frac * scale)
    clip_h = int(256 * clip_h_frac * scale)
    clip_x0 = int((256*scale - clip_w)/2)
    clip_x1 = clip_x0 + clip_w
    clip_y0 = int((by0*256 - clip_h*0.45) * scale)  # overlap top edge
    if clip_y0 < int(6*scale):
        clip_y0 = int(6*scale)
    clip_y1 = clip_y0 + clip_h
    clip_r = int(6*scale) if size >= 48 else int(5*scale) if size >= 32 else int(3*scale)

    rounded_rect(draw, [clip_x0, clip_y0, clip_x1, clip_y1],
                 radius=clip_r, fill=COLORS["clip_top"], outline=None)

    # Clip highlight
    if size >= 32:
        hl_x0 = clip_x0 + int(12*scale)
        hl_x1 = clip_x1 - int(12*scale)
        hl_y0 = clip_y0 + int(5*scale)
        hl_y1 = hl_y0 + int(4*scale)
        if hl_x1 > hl_x0:
            try:
                draw.rounded_rectangle([hl_x0, hl_y0, hl_x1, hl_y1], radius=int(2*scale), fill=COLORS["clip_highlight"])
            except:
                pass
        # Notch
        notch_w = int(22*scale)
        notch_h = int(7*scale)
        notch_x0 = int((256*scale - notch_w)/2)
        notch_x1 = notch_x0 + notch_w
        notch_y0 = clip_y1 - int(1*scale)
        notch_y1 = notch_y0 + notch_h
        try:
            draw.rounded_rectangle([notch_x0, notch_y0, notch_x1, notch_y1], radius=int(3*scale), fill=COLORS["clipboard_bg"])
        except:
            pass

    # Lines
    if size >= 20:
        if size <= 24:
            # For tiny, just 2 lines, thicker
            line_h = int(5*scale)
            gap = int(10*scale)
            w = int(80*scale)
            x0 = int((256*scale - w)/2)
            x1 = x0 + w
            base_y = int((body_y0 + body_y1)/2 - gap/2)
            for i in range(2):
                y0 = base_y + i*(line_h+gap)
                y1 = y0 + line_h
                col = COLORS["line_mid"] if i==1 else COLORS["line"]
                try:
                    draw.rounded_rectangle([x0, y0, x1, y1], radius=line_h//2, fill=col)
                except:
                    draw.rectangle([x0, y0, x1, y1], fill=col)
        else:
            line_h = int(11*scale) if size >= 64 else int(9*scale) if size >= 32 else int(7*scale)
            gap = int(18*scale) if size >= 48 else int(14*scale) if size >= 32 else int(10*scale)
            w_full = int(95*scale)
            w_half = int(65*scale)
            x0 = int((256*scale - w_full)/2)
            x1_full = x0 + w_full
            x1_half = x0 + w_half
            base_y = int(body_y0 + 38*scale) if size >= 48 else int(body_y0 + 30*scale)
            for i, is_half in enumerate([False, False, True]):
                y0 = base_y + i*(line_h+gap)
                y1 = y0 + line_h
                x1 = x1_half if is_half else x1_full
                col = COLORS["line_mid"] if i==1 else COLORS["line_light"] if i==2 else COLORS["line"]
                # Make middle line slightly darker and bolder
                if i==1 and size >= 32:
                    # slightly thicker
                    y0 -= 1
                    y1 += 1
                try:
                    draw.rounded_rectangle([x0, y0, x1, y1], radius=line_h//2, fill=col)
                except:
                    draw.rectangle([x0, y0, x1, y1], fill=col)

    # Blue circle with lightning
    if size >= 16:
        if size <= 20:
            # For 16-20, circle is relatively large to be visible
            r = int(11*scale) if size==16 else int(13*scale)
            cx = body_x1 - int(6*scale)
            cy = body_y1 - int(6*scale)
        elif size <= 24:
            r = int(16*scale)
            cx = body_x1 - int(10*scale)
            cy = body_y1 - int(10*scale)
        elif size <= 32:
            r = int(20*scale)
            cx = body_x1 - int(12*scale)
            cy = body_y1 - int(12*scale)
        elif size <= 48:
            r = int(28*scale)
            cx = body_x1 - int(14*scale)
            cy = body_y1 - int(14*scale)
        else:
            r = int(42*scale)
            cx = body_x1 - int(18*scale)
            cy = body_y1 - int(18*scale)

        # White outline for contrast
        outer = r + max(1, int(2*scale))
        # Use white with slight transparency for soft edge
        draw.ellipse([cx-outer, cy-outer, cx+outer, cy+outer], fill=(255,255,255,255))
        # Subtle outer shadow for circle
        if size >= 32:
            draw.ellipse([cx-outer+1, cy-outer+1, cx+outer+1, cy+outer+1], fill=(0,0,0,0))  # placeholder

        # Blue circle with gradient effect (simple: darker edge)
        # Draw base blue
        draw.ellipse([cx-r, cy-r, cx+r, cy+r], fill=COLORS["accent"])
        # Add inner highlight for 3D (top-left lighter)
        if size >= 32:
            # Inner highlight ellipse offset
            hl_r = int(r*0.7)
            hl_cx = cx - int(r*0.15)
            hl_cy = cy - int(r*0.2)
            # Use lighter blue with transparency
            # Draw a smaller lighter circle as highlight
            # We'll draw an ellipse with lighter color and low alpha by blending
            # For simplicity, just draw a small white dot highlight
            if size >= 48:
                dot_r = int(r*0.18)
                dot_x = cx - int(r*0.3)
                dot_y = cy - int(r*0.35)
                draw.ellipse([dot_x-dot_r, dot_y-dot_r, dot_x+dot_r, dot_y+dot_r], fill=(255,255,255,60))

        # Lightning
        if size >= 16:
            # Simplify lightning for tiny sizes: just a vertical bolt with less zigzag
            if size <= 20:
                # Very simple bolt: 4 points
                pts = [
                    ( 0.0*r, -0.5*r),
                    ( 0.25*r, 0.0*r),
                    (-0.05*r, 0.05*r),
                    ( 0.15*r, 0.5*r),
                    (-0.1*r, 0.1*r),
                    (-0.2*r, 0.1*r),
                    (0.0*r, -0.5*r),
                ]
            elif size <= 24:
                pts = [
                    ( 0.0*r, -0.55*r),
                    ( 0.28*r, -0.05*r),
                    ( 0.06*r, 0.02*r),
                    ( 0.20*r, 0.50*r),
                    (-0.06*r, 0.10*r),
                    (-0.18*r, 0.10*r),
                    (0.0*r, -0.55*r),
                ]
            else:
                pts = [
                    ( 0.00*r, -0.58*r),
                    ( 0.30*r, -0.10*r),
                    ( 0.08*r, -0.02*r),
                    ( 0.24*r,  0.58*r),
                    (-0.04*r,  0.12*r),
                    (-0.18*r,  0.12*r),
                    ( 0.00*r, -0.58*r),
                ]
            abs_pts = [(cx + x, cy + y) for x,y in pts]
            draw.polygon(abs_pts, fill=COLORS["lightning"])
            # Add subtle shadow under lightning for depth on large sizes
            if size >= 64:
                # slightly offset darker lightning underneath
                shadow_pts = [(x+1, y+1) for x,y in abs_pts]
                # draw underneath first
                # We already drew, so for next time we would draw shadow first
                pass

    if output_path:
        img.save(output_path, "PNG")
    return img

def generate_all_v2():
    base_dir = os.path.dirname(os.path.abspath(__file__))
    sizes = [16, 20, 24, 32, 48, 64, 128, 256, 512]
    for s in sizes:
        img = draw_icon(s)
        out = os.path.join(base_dir, f"icon_{s}.png")
        img.save(out, "PNG")
        print(f"Generated {out}")

    img256 = draw_icon(256)
    img256.save(os.path.join(base_dir, "icon.png"), "PNG")
    print("Generated icon.png")

    ico_sizes = [16, 20, 24, 32, 48, 64, 256]
    ico_images = [draw_icon(s) for s in ico_sizes]
    ico_path = os.path.join(base_dir, "icon.ico")
    largest = ico_images[-1]
    largest.save(ico_path, format="ICO", sizes=[(im.width, im.height) for im in ico_images])
    print(f"Generated {ico_path}")

    # Preview sheet
    sheet_w, sheet_h = 900, 500
    sheet = Image.new("RGBA", (sheet_w, sheet_h), (248, 250, 252, 255))
    draw = ImageDraw.Draw(sheet)
    # Right half dark
    draw.rectangle([sheet_w//2, 0, sheet_w, sheet_h], fill=(17, 24, 39, 255))
    # Add subtle grid
    try:
        from PIL import ImageFont
        font = ImageFont.load_default()
        font_bold = font
    except:
        font = None

    # Titles
    if font:
        draw.text((24, 18), "FastPaste  •  Light", fill=(30,41,59,255), font=font)
        draw.text((sheet_w//2+24, 18), "FastPaste  •  Dark", fill=(255,255,255,255), font=font)
        draw.text((24, 38), "Clipboard + Lightning  •  High contrast, works at 16px", fill=(100,116,139,255), font=font)
        draw.text((sheet_w//2+24, 38), "Clipboard + Lightning  •  High contrast, works at 16px", fill=(148,163,184,255), font=font)

    # Draw sizes
    y = 70
    for s in [16, 20, 24, 32, 48, 64, 128, 256]:
        if s == 256:
            # scale down 256 to 96 for preview to fit
            im = draw_icon(256).resize((96,96), Image.LANCZOS)
            s_display = 96
            label = "256→96"
        else:
            im = draw_icon(s)
            s_display = s
            label = f"{s}x{s}"
        # Light
        sheet.alpha_composite(im, (24, y))
        # Dark
        sheet.alpha_composite(im, (sheet_w//2+24, y))
        if font:
            draw.text((24 + s_display + 12, y + s_display//2 - 7), label, fill=(51,65,85,255), font=font)
            draw.text((sheet_w//2+24 + s_display + 12, y + s_display//2 - 7), label, fill=(203,213,225,255), font=font)
            # Size indicator
            desc = "tray" if s <= 32 else "exe" if s >= 64 else "mid"
            draw.text((24 + s_display + 60, y + s_display//2 - 7), desc, fill=(148,163,184,255), font=font)
            draw.text((sheet_w//2+24 + s_display + 60, y + s_display//2 - 7), desc, fill=(100,116,139,255), font=font)
        y += s_display + 14
        if y > sheet_h - 20:
            break

    # Also show on colored backgrounds (Windows 11 style)
    # Add a small strip at bottom showing on blue
    # Bottom bar
    # draw.rectangle([0, sheet_h-40, sheet_w, sheet_h], fill=(37,99,235,255))
    # if font:
    #     draw.text((24, sheet_h-26), "On accent", fill=(255,255,255,255), font=font)

    sheet_path = os.path.join(base_dir, "preview_v2.png")
    sheet.convert("RGB").save(sheet_path, "PNG")
    print(f"Generated preview {sheet_path}")

if __name__ == "__main__":
    generate_all_v2()
