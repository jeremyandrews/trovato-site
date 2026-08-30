#!/usr/bin/env python3
"""Generate the site's illustrations.

Run `python3 gen_art.py` in this directory and every SVG beside this file is
rewritten from the geometry below. This is the same arrangement the brand
sheet uses (`assets/brand/gen_final.py` in the kernel repository): the script
is the source of truth and the SVGs are its output, so a tweak is an edit
here and a re-run, never a hand-edit of a path.

── The illustration language ────────────────────────────────────────────────

Every piece speaks the language of the mark: monoline strokes with round caps
and joins, at a weight proportional to the mark's 30/240, and exactly one
filled clay circle per piece — the found-dot, the thing you were looking for.
Nothing else is saturated. Text inside an illustration is greeked (drawn as
rounded strokes), which keeps every piece language-neutral: the same file
serves the English and the Italian page.

── Color, and how dark mode works ───────────────────────────────────────────

Each standalone SVG carries its own <style> with a `prefers-color-scheme`
media query, so one file follows the reader's scheme even when it is loaded
through <img>. That behaviour is exercised by the same screenshot gate that
checks the rest of the site: scripts/shoot.mjs renders every page in both
schemes, so an illustration that ignored the scheme would show up there as a
cream rectangle on a dark page.

The palette is the brand's, verbatim — nothing here invents a color:

  Ink #221B16, Clay #B14B2E, Clay Light #E08963, Peach #FFD9A8,
  Cream #FAF5EF, plus the two derived surfaces tokens.css already uses.

The dot is Clay on light and Clay Light on dark, exactly as BRAND.md says a
dot on a dark background must be.

── The inline variant ───────────────────────────────────────────────────────

One drawing is inlined rather than served: the hero, in
templates/elements/item--front_page.html, so it can paint with the site's
`--art-*` tokens and the stylesheet can animate its two named parts. The
generator writes that variant to `inline/hero-found.svg.html`; the suffix is
the reminder that it is a fragment for a template, not a file to serve.
Everything else is loaded through <img>, where the standalone file's own
media query does the theming — and where an inline `var(--…)` inside served
config prose would trip the dash test in checks/tests/content.rs.
"""

import os

# ── Palette ──────────────────────────────────────────────────────────────────

INK = "#221B16"
CLAY = "#B14B2E"
CLAY_LIGHT = "#E08963"
PEACH = "#FFD9A8"
CREAM = "#FAF5EF"

# Light-scheme roles.
L = {
    "stroke": INK,          # primary line
    "soft": "#8E7A66",      # secondary line (tokens.css --border-control)
    "faint": "#E0D2C0",     # hairline (tokens.css --border)
    "dot": CLAY,            # the found-dot
    "accent": CLAY,         # accent stroke
    "fill": "#F1E8DC",      # sunken panel (tokens.css --surface-sunken)
    "panel": CREAM,         # raised panel / browser chrome
    "peach": PEACH,         # warm highlight fill
}
# Dark-scheme roles. Warm, never gray, matching tokens.css.
D = {
    "stroke": CREAM,
    "soft": "#A2917F",
    "faint": "#3A2E26",
    "dot": CLAY_LIGHT,
    "accent": CLAY_LIGHT,
    "fill": "#221B16",
    "panel": "#2A211B",
    "peach": "#4A3423",     # peach's job (a warm wash) at a dark value
    "invert": CREAM,        # the *other* scheme's page, for banner-a11y
    "oninvert": INK,        # ink on that page
    "oninvert2": "#8E7A66",  # its muted line
}
L["invert"] = "#26201A"
L["oninvert"] = CREAM
L["oninvert2"] = "#A2917F"
# Inline variant: the same roles as CSS custom properties, resolved by
# tokens.css so the front page's diagrams always agree with the site.
V = {k: f"var(--art-{k}, {v})" for k, v in L.items()}

ROLES = ["stroke", "soft", "faint", "dot", "accent", "fill", "panel", "peach",
         "invert", "oninvert", "oninvert2"]


def style_block() -> str:
    light = " ".join(f".a-{r}{{color:{L[r]}}}" for r in ROLES)
    dark = " ".join(f".a-{r}{{color:{D[r]}}}" for r in ROLES)
    return (
        f"<style>{light}"
        f"@media (prefers-color-scheme: dark){{{dark}}}</style>"
    )


def svg(name, w, h, bodies, title, inline=False):
    """Write name.svg, and inline/name.svg.html when a template consumes it.

    `bodies` is the (standalone, inline) pair that `both` returns. Only the
    hero asks for its inline variant: everything else is loaded through
    <img>, where the standalone file's own media query does the theming —
    and where a `var(--…)` inside served config prose would trip the dash
    test in checks/tests/content.rs.
    """
    body, inline_body = bodies
    head = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" '
        f'role="img" aria-label="{title}">'
    )
    out = f"{head}{style_block()}{body}</svg>\n"
    with open(f"{name}.svg", "w") as f:
        f.write(out)
    if not inline:
        return
    os.makedirs("inline", exist_ok=True)
    inline = inline_body if inline_body is not None else body
    # The inline fragment is decorative inside a figure that carries the prose;
    # aria-hidden keeps it from being announced twice.
    ihead = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" '
        f'aria-hidden="true" class="art">'
    )
    with open(f"inline/{name}.svg.html", "w") as f:
        f.write(f"{ihead}{inline}</svg>\n")


class Pen:
    """Emit SVG in either standalone (class-driven) or inline (var) color."""

    def __init__(self, palette=None):
        self.parts = []
        self.vars = palette  # None => standalone classes; dict => inline vars

    def _paint(self, role, attr):
        if self.vars is None:
            return f'class="a-{role}" {attr}="currentColor"'
        return f'{attr}="{self.vars[role]}"'

    def stroke_attrs(self, role, width):
        return (
            f"{self._paint(role, 'stroke')} fill=\"none\" "
            f'stroke-width="{width}" stroke-linecap="round" '
            f'stroke-linejoin="round"'
        )

    def path(self, d, role="stroke", w=10, cls=None, attrs=None):
        extra = ""
        if cls and self.vars is not None:
            # Class hooks exist only in inline fragments, where the page's own
            # stylesheet can reach them (the front page animates two of them).
            extra = f' class="{cls}"'
        if attrs and self.vars is not None:
            extra += f" {attrs}"
        self.parts.append(
            f'<path d="{d}"{extra} {self.stroke_attrs(role, w)}/>')

    def line(self, x1, y1, x2, y2, role="stroke", w=10, dash=None):
        d = f' stroke-dasharray="{dash}"' if dash else ""
        self.parts.append(
            f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" '
            f"{self.stroke_attrs(role, w)}{d}/>"
        )

    def rect(self, x, y, w_, h, r=0, role=None, fill=None, sw=10):
        bits = [f'<rect x="{x}" y="{y}" width="{w_}" height="{h}" rx="{r}"']
        if fill is not None:
            bits.append(self._paint(fill, "fill"))
        else:
            bits.append('fill="none"')
        if role is not None:
            bits.append(
                f"{self._paint(role, 'stroke')} stroke-width=\"{sw}\" "
                'stroke-linejoin="round"'
            )
        self.parts.append(" ".join(bits) + "/>")

    def circle(self, cx, cy, r, role=None, fill=None, sw=10):
        bits = [f'<circle cx="{cx}" cy="{cy}" r="{r}"']
        if fill is not None:
            bits.append(self._paint(fill, "fill"))
        else:
            bits.append('fill="none"')
        if role is not None:
            bits.append(f"{self._paint(role, 'stroke')} stroke-width=\"{sw}\"")
        self.parts.append(" ".join(bits) + "/>")

    def dot(self, cx, cy, r=14, cls=None):
        """The found-dot. One per piece."""
        if cls and self.vars is not None:
            self.parts.append(
                f'<circle cx="{cx}" cy="{cy}" r="{r}" class="{cls}" '
                f'{self._paint("dot", "fill")}/>')
        else:
            self.circle(cx, cy, r, fill="dot")

    def greek(self, x, y, widths, role="soft", w=8, gap=18):
        """Greeked text: one rounded stroke per 'word'."""
        cx = x
        for width in widths:
            self.line(cx, y, cx + width, y, role=role, w=w)
            cx += width + gap

    def html(self):
        return "".join(self.parts)


def both(draw):
    """Run a drawing function against both pens; return (standalone, inline)."""
    a, b = Pen(None), Pen(V)
    draw(a)
    draw(b)
    return a.html(), b.html()


# ── Shared fixtures ──────────────────────────────────────────────────────────

def browser_frame(p, x, y, w, h, bar=54):
    """A browser window. Returns the content box (cx, cy, cw, ch)."""
    p.rect(x, y, w, h, r=18, fill="panel")
    p.rect(x, y, w, h, r=18, role="soft", sw=8)
    p.line(x + 4, y + bar, x + w - 4, y + bar, role="faint", w=6)
    for i in range(3):
        p.circle(x + 34 + i * 34, y + bar / 2, 9, role="soft", sw=7)
    # address pill
    p.rect(x + 138, y + bar / 2 - 13, w - 180, 26, r=13, role="faint", sw=6)
    p.greek(x + 158, y + bar / 2, [w * 0.22], role="soft", w=7)
    return x + 28, y + bar + 28, w - 56, h - bar - 56


def terminal_frame(p, x, y, w, h, bar=54):
    p.rect(x, y, w, h, r=18, fill="fill")
    p.rect(x, y, w, h, r=18, role="soft", sw=8)
    p.line(x + 4, y + bar, x + w - 4, y + bar, role="faint", w=6)
    for i in range(3):
        p.circle(x + 34 + i * 34, y + bar / 2, 9, role="soft", sw=7)
    return x + 32, y + bar + 40, w - 64, h - bar - 64


# ── 1. Hero: the found moment ────────────────────────────────────────────────
# Scattered outline shapes drift in the field; the t's scoop sweeps under
# them and cradles the one that matters. Trovato: found.

def hero(p):
    # The search: one long faint path that wanders the whole field and ends
    # exactly where the dot rests. Everything it passed, it passed by.
    p.path(
        "M 46 76 C 140 20, 210 140, 300 92 S 470 20, 560 84 "
        "S 700 190, 620 240 S 480 320, 482 272",
        role="faint", w=6, cls="hero-search", attrs='pathLength="1"')
    # Drifting, unfound: outline circles and rounded squares, quiet.
    drift = [
        (70, 84, "c", 15), (206, 52, "s", 24), (330, 96, "c", 11),
        (472, 44, "s", 20), (600, 90, "c", 16), (716, 60, "s", 22),
        (110, 208, "s", 20), (250, 170, "c", 13), (560, 190, "c", 10),
        (668, 176, "s", 18), (78, 322, "c", 9), (188, 300, "s", 15),
        (640, 300, "c", 13), (716, 250, "c", 8),
    ]
    for x, y, kind, r in drift:
        if kind == "c":
            p.circle(x, y, r, role="soft", sw=8)
        else:
            p.rect(x - r, y - r, 2 * r, 2 * r, r=8, role="soft", sw=8)
    # The stem and scoop, the mark's own geometry writ large.
    p.path("M 400 128 L 400 268 Q 400 344 480 344", role="stroke", w=34)
    p.path("M 336 196 L 462 196", role="stroke", w=34)
    # The found-dot, held in the nook where the search ends.
    p.dot(482, 272, 24, cls="hero-dot")


svg("hero-found", 780, 400, both(hero),
    title="Scattered outline shapes, and a large lowercase t holding one "
          "filled clay dot in the curve of its foot: found.", inline=True)


# ── 2. Diagram: the manifest is the whole of what it may reach ───────────────

def d_sandbox(p):
    # Plugin box, left.
    p.rect(40, 90, 220, 220, r=20, fill="fill")
    p.rect(40, 90, 220, 220, r=20, role="stroke", sw=10)
    p.greek(76, 140, [120], role="stroke", w=9)
    # The manifest: three declared lines inside the plugin.
    p.greek(76, 190, [90], role="soft")
    p.greek(76, 228, [122], role="soft")
    p.greek(76, 266, [66], role="soft")
    # Kernel wall, right.
    p.line(560, 60, 560, 340, role="stroke", w=12)
    p.greek(596, 96, [90], role="stroke", w=9)
    # Three declared interfaces: pass through gates in the wall.
    for i, y in enumerate((150, 214)):
        p.line(260, y, 610, y, role="accent", w=10)
        p.circle(560, y, 16, fill="panel")
        p.circle(560, y, 16, role="accent", sw=9)
    # The undeclared call: stops dead at the wall.
    p.line(260, 292, 508, 292, role="soft", w=10, dash="2 26")
    p.line(524, 272, 524, 312, role="soft", w=11)
    # What the kernel holds beyond the wall.
    p.greek(596, 150, [70, 40], role="soft")
    p.greek(596, 214, [96], role="soft")
    p.greek(596, 292, [56], role="faint")
    # The found-dot rides the first declared interface.
    p.dot(374, 150, 15)


svg("diagram-sandbox", 720, 400, both(d_sandbox),
    title="A plugin's declared interfaces pass through gates in the kernel "
          "wall; an undeclared call stops at the wall.")


# ── 3. Diagram: twenty fields, one row ───────────────────────────────────────

def d_onerow(p):
    # Left: the old way — a scatter of per-field tables, joined and joined.
    tables = [(48, 60), (150, 96), (58, 170), (166, 204), (70, 282)]
    for x, y in tables:
        p.rect(x, y, 84, 58, r=10, role="soft", sw=8)
        p.line(x + 10, y + 20, x + 74, y + 20, role="faint", w=6)
        p.greek(x + 12, y + 42, [40], role="faint", w=6)
    # Join spaghetti.
    p.path("M 132 89 C 200 60, 160 120, 150 118", role="faint", w=6)
    p.path("M 100 118 C 90 150, 96 150, 100 170", role="faint", w=6)
    p.path("M 142 199 C 150 220, 158 216, 166 226", role="faint", w=6)
    p.path("M 112 228 C 120 260, 108 258, 112 282", role="faint", w=6)
    # The arrow of the argument.
    p.path("M 300 200 L 396 200 M 368 172 L 396 200 L 368 228",
           role="stroke", w=11)
    # Right: one row. A single wide capsule with twenty keys.
    p.rect(440, 148, 240, 104, r=22, fill="fill")
    p.rect(440, 148, 240, 104, r=22, role="stroke", sw=11)
    for r_ in range(2):
        for c in range(10):
            x = 466 + c * 20
            y = 186 + r_ * 30
            p.circle(x, y, 5.5, role="soft", sw=5)
    # The found-dot is one of the twenty: the field you asked for, right there.
    p.dot(586, 186, 8)


svg("diagram-onerow", 720, 400, both(d_onerow),
    title="Five joined per-field tables on the left; on the right one row "
          "holding twenty keys, one of them the clay dot.")


# ── 4. Diagram: a query is a definition ──────────────────────────────────────

def d_gather(p):
    # The definition: a stacked card of declared parts.
    p.rect(52, 72, 250, 256, r=20, fill="fill")
    p.rect(52, 72, 250, 256, r=20, role="stroke", sw=10)
    p.greek(84, 116, [130], role="stroke", w=9)
    for y, w_ in ((166, [90, 60]), (208, [140]), (250, [70, 80]), (292, [110])):
        p.greek(84, y, w_, role="soft")
    # Flow to the display.
    p.path("M 302 200 L 380 200 M 354 174 L 380 200 L 354 226",
           role="stroke", w=11)
    # The display: a clean listing.
    p.rect(420, 60, 250, 280, r=20, fill="panel")
    p.rect(420, 60, 250, 280, r=20, role="soft", sw=9)
    ys = (116, 172, 228, 284)
    for i, y in enumerate(ys):
        p.greek(478, y - 12, [120 if i % 2 == 0 else 96], role="stroke", w=8)
        p.greek(478, y + 14, [150], role="faint", w=6)
        if i < len(ys) - 1:
            p.line(446, y + 34, 644, y + 34, role="faint", w=5)
    # The found-dot marks the first result.
    p.dot(450, 108, 11)


svg("diagram-gather", 720, 400, both(d_gather),
    title="A query definition card flowing into a rendered listing; the "
          "first result carries the clay dot.")


# ── 5. Screenshot: the admin content list ────────────────────────────────────

def shot_admin(p):
    cx, cy, cw, ch = browser_frame(p, 20, 20, 760, 480)
    # Sidebar.
    p.rect(cx, cy, 150, ch, r=14, fill="fill")
    for i in range(4):
        p.greek(cx + 22, cy + 40 + i * 46, [86 - (i % 2) * 24], role="soft", w=8)
    # Heading + Add button.
    p.greek(cx + 186, cy + 34, [170], role="stroke", w=11)
    p.rect(cx + cw - 128, cy + 12, 128, 44, r=22, fill="peach")
    p.greek(cx + cw - 100, cy + 34, [72], role="stroke", w=8)
    # Rows: title, stage chip, date.
    for i in range(5):
        y = cy + 96 + i * 62
        p.line(cx + 178, y + 40, cx + cw, y + 40, role="faint", w=5)
        p.greek(cx + 186, y, [150 + (i * 37) % 90], role="stroke", w=9)
        chip_x = cx + cw - 240
        p.rect(chip_x, y - 15, 84, 30, r=15, role="soft", sw=6)
        p.greek(cx + cw - 92, y, [70], role="faint", w=6)
    # The found-dot: the row you were looking for, marked published.
    p.dot(cx + 165, cy + 96 + 62, 9)


svg("shot-admin-content", 800, 520, both(shot_admin),
    title="A stylized screenshot of the Trovato admin content list: sidebar, "
          "rows of titles with stage chips and dates.")


# ── 6. Screenshot: the Gather builder ────────────────────────────────────────

def shot_gather(p):
    cx, cy, cw, ch = browser_frame(p, 20, 20, 760, 480)
    # Left: the builder column.
    p.greek(cx + 8, cy + 30, [180], role="stroke", w=11)
    for i, y in enumerate(range(0, 3)):
        ry = cy + 74 + i * 78
        p.rect(cx + 8, ry, 330, 58, r=14, fill="fill")
        p.greek(cx + 30, ry + 22, [64], role="soft", w=7)
        p.greek(cx + 30, ry + 42, [120 + (i * 53) % 80], role="stroke", w=8)
    # Add-filter ghost row.
    p.rect(cx + 8, cy + 74 + 3 * 78, 330, 58, r=14, role="faint", sw=7)
    p.line(cx + 160, cy + 90 + 3 * 78, cx + 186, cy + 90 + 3 * 78,
           role="soft", w=8)
    p.line(cx + 173, cy + 77 + 3 * 78, cx + 173, cy + 103 + 3 * 78,
           role="soft", w=8)
    # Right: the live preview.
    px = cx + 386
    p.rect(px, cy + 10, cw - 386 - 8, ch - 20, r=16, fill="panel")
    p.rect(px, cy + 10, cw - 386 - 8, ch - 20, r=16, role="faint", sw=7)
    p.greek(px + 28, cy + 48, [110], role="soft", w=7)
    for i in range(4):
        y = cy + 100 + i * 74
        p.greek(px + 28, y, [150 - (i * 29) % 60], role="stroke", w=9)
        p.greek(px + 28, y + 26, [190], role="faint", w=6)
    # The found-dot: the preview's first result.
    p.dot(px + 10, cy + 96, 8)


svg("shot-gather-builder", 800, 520, both(shot_gather),
    title="A stylized screenshot of the Gather query builder: filter cards "
          "on the left, a live preview listing on the right.")


# ── 7. Screenshot: the installer ─────────────────────────────────────────────

def shot_installer(p):
    cx, cy, cw, ch = browser_frame(p, 20, 20, 760, 480)
    mx = cx + cw / 2
    # The tile: the favicon's flat clay square, white t, peach dot — drawn
    # mono here (soft strokes) so the found-dot stays unique.
    tx, ty, ts = mx - 44, cy + 8, 88
    p.rect(tx, ty, ts, ts, r=20, role="stroke", sw=9)
    p.path(f"M {tx + 38} {ty + 20} L {tx + 38} {ty + 52} "
           f"Q {tx + 38} {ty + 70} {tx + 57} {ty + 70}",
           role="stroke", w=10)
    p.line(tx + 23, ty + 36, tx + 52, ty + 36, role="stroke", w=10)
    p.dot(tx + 57, ty + 53, 7)
    # Welcome line.
    p.greek(mx - 105, cy + 138, [210], role="stroke", w=11)
    # Two fields and the button.
    for i, y in enumerate((cy + 186, cy + 262)):
        p.greek(mx - 170, y - 22, [90], role="soft", w=7)
        p.rect(mx - 170, y, 340, 46, r=12, role="soft", sw=7)
        p.greek(mx - 148, y + 23, [120 + i * 40], role="faint", w=7)
    p.rect(mx - 90, cy + 342, 180, 50, r=25, fill="peach")
    p.greek(mx - 52, cy + 367, [104], role="stroke", w=9)


svg("shot-installer", 800, 520, both(shot_installer),
    title="A stylized screenshot of the Trovato installer: the tile mark, "
          "two form fields, and a single button.")


# ── 8. Screenshot: the terminal ──────────────────────────────────────────────

def shot_terminal(p):
    cx, cy, cw, ch = terminal_frame(p, 20, 20, 760, 400)
    lines = [
        ("prompt", [26, 220]),          # git clone …
        ("prompt", [26, 96]),           # cd trovato
        ("prompt", [26, 180]),          # cp .env.example .env
        ("prompt", [26, 300]),          # docker compose --profile full up
        ("out", [150]),
        ("out", [240, 60]),
    ]
    y = cy
    for kind, widths in lines:
        if kind == "prompt":
            p.line(cx, y, cx + 14, y, role="accent", w=8)
            p.greek(cx + 34, y, widths, role="stroke", w=8)
        else:
            p.greek(cx + 34, y, widths, role="soft", w=7)
        y += 42
    # The found-dot: the ready line at the bottom.
    p.dot(cx + 7, y, 8)
    p.greek(cx + 34, y, [130, 70], role="stroke", w=8)


svg("shot-terminal", 800, 440, both(shot_terminal),
    title="A stylized terminal: four prompt lines, quiet output, and a clay "
          "dot beside the line that says the site is up.")


# ── 9. Banner: the open door (community) ─────────────────────────────────────

def banner_community(p):
    # The doorway, and the light inside it.
    p.rect(310, 78, 160, 262, r=6, fill="peach")
    # The frame: a post-and-lintel path, open at the floor.
    p.path("M 300 352 L 300 68 L 480 68 L 480 352", role="stroke", w=13)
    # The door leaf, swung open toward the reader.
    p.path("M 300 68 L 214 110 L 214 366 L 300 340", role="stroke", w=11)
    p.circle(238, 240, 8, role="stroke", sw=8)
    # The floor line.
    p.line(120, 352, 560, 352, role="faint", w=8)
    # Light spilling out.
    for x1, y1, x2, y2 in ((498, 132, 556, 108), (502, 210, 572, 210),
                           (498, 288, 556, 312)):
        p.line(x1, y1, x2, y2, role="soft", w=8)
    # The dot, arriving at the threshold.
    p.dot(390, 322, 15)


svg("banner-community", 640, 420, both(banner_community),
    title="A door standing open with light spilling out, and the clay dot "
          "on the path to the threshold.")


# ── 10. Banner: contrast measured (accessibility) ────────────────────────────

def banner_a11y(p):
    # The same page twice: once in this scheme, once in the other one.
    p.rect(70, 70, 220, 260, r=18, fill="panel")
    p.rect(70, 70, 220, 260, r=18, role="soft", sw=9)
    p.greek(102, 118, [120], role="stroke", w=10)
    p.greek(102, 160, [156], role="soft")
    p.greek(102, 196, [130], role="soft")
    p.rect(350, 70, 220, 260, r=18, fill="invert")
    p.rect(350, 70, 220, 260, r=18, role="soft", sw=9)
    p.greek(382, 118, [120], role="oninvert", w=10)
    p.greek(382, 160, [156], role="oninvert2")
    p.greek(382, 196, [130], role="oninvert2")
    # The focus ring, drawn as the visible thing it is, in both schemes.
    p.rect(96, 240, 130, 52, r=12, role="accent", sw=10)
    p.rect(376, 240, 130, 52, r=12, role="accent", sw=10)
    # Measured, both of them: a check below the pair.
    p.path("M 286 374 L 308 394 L 352 344", role="stroke", w=12)
    p.dot(320, 300, 11)


svg("banner-a11y", 640, 420, both(banner_a11y),
    title="The same page in light and dark, each with a visible focus ring, "
          "joined by a check mark: every pairing measured.")


# ── 11. Banner: the crate stack (rust) ───────────────────────────────────────

def banner_rust(p):
    # A stack of crates, honestly load-bearing.
    boxes = [(120, 268, 280), (80, 196, 170), (270, 196, 170),
             (160, 124, 200)]
    for x, y, w_ in boxes:
        p.rect(x, y, w_, 64, r=12, role="stroke", sw=10)
        p.greek(x + 26, y + 32, [w_ - 92], role="soft")
    # The gauge of a checked build, clear of the stack.
    p.circle(520, 232, 62, role="soft", sw=9)
    p.path("M 490 234 L 512 256 L 552 206", role="stroke", w=12)
    # The found-dot capping the stack.
    p.dot(260, 96, 13)


svg("banner-rust", 640, 420, both(banner_rust),
    title="A stack of crates with a passing check gauge beside it, and the "
          "clay dot resting on top.")


# ── 12. Icons: the front page's three claims, at card size ───────────────────

def icon_onerow(p):
    p.rect(14, 34, 92, 52, r=16, fill="fill")
    p.rect(14, 34, 92, 52, r=16, role="stroke", sw=9)
    for c in range(5):
        x = 33 + c * 14
        p.circle(x, 52, 3.6, role="soft", sw=4)
        p.circle(x, 69, 3.6, role="soft", sw=4)
    p.dot(75, 52, 5.5)


def icon_sandbox(p):
    p.rect(14, 22, 62, 76, r=14, fill="fill")
    p.rect(14, 22, 62, 76, r=14, role="stroke", sw=9)
    p.greek(30, 44, [30], role="soft", w=6)
    p.greek(30, 62, [22], role="soft", w=6)
    p.line(76, 80, 104, 80, role="accent", w=7)
    p.line(96, 42, 96, 60, role="stroke", w=8)
    p.dot(86, 80, 5.5)


def icon_gather(p):
    p.rect(14, 18, 92, 84, r=14, fill="panel")
    p.rect(14, 18, 92, 84, r=14, role="soft", sw=8)
    for i, y in enumerate((40, 60, 80)):
        p.greek(40, y, [46 - (i % 2) * 12], role="soft", w=6)
    p.dot(28, 40, 5.5)


svg("icon-onerow", 120, 120, both(icon_onerow),
    title="A single row of round fields, one of them the clay dot.")
svg("icon-sandbox", 120, 120, both(icon_sandbox),
    title="A plugin box with one declared line reaching out and a wall "
          "stopping everything else.")
svg("icon-gather", 120, 120, both(icon_gather),
    title="A small listing with the clay dot beside its first row.")


if __name__ == "__main__":
    print("regenerated:", ", ".join(sorted(
        f for f in os.listdir(".") if f.endswith(".svg"))))
