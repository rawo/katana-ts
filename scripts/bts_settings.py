#!/usr/bin/env python3
"""Write docs/settings.md: every KATANA Gen 3 setting known to BOSS TONE STUDIO.

Reads three data tables from the user's own Tone Studio installation (address_map.js,
export/item.json, export/layout.div) and joins them; no Tone Studio logic is copied.

    scripts/bts_settings.py [--html DIR] [--out docs/settings.md]

DIR defaults to the macOS app bundle's html folder, else the Wine prefix's.
"""
import argparse
import json
import re
import sys
from html.parser import HTMLParser
from pathlib import Path

VOID = {"br", "img", "input", "hr", "meta", "link", "source", "wbr", "col", "area", "base", "embed", "param", "track"}


class El:
    def __init__(self, tag, attrs, parent):
        self.tag = tag
        self.attrs = dict(attrs)
        self.parent = parent
        self.children = []
        self.text = []

    @property
    def id(self):
        return self.attrs.get("id")

    def classes(self):
        return self.attrs.get("class", "").split()

    def ancestors(self):
        p = self.parent
        while p is not None:
            yield p
            p = p.parent

    def all_text(self):
        out = list(self.text)
        for c in self.children:
            out.append(c.all_text())
        return "".join(out)

    def first_p_text(self):
        for c in self.children:
            if c.tag == "p":
                return c.all_text().strip()
        return None


class P(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.root = El("root", [], None)
        self.cur = self.root
        self.by_id = {}
        self.order = []

    def handle_starttag(self, tag, attrs):
        if tag == "br":
            self.cur.text.append(" ")
            return
        e = El(tag, attrs, self.cur)
        self.cur.children.append(e)
        self.order.append(e)
        if e.id:
            self.by_id.setdefault(e.id, e)
        if tag not in VOID:
            self.cur = e

    def handle_endtag(self, tag):
        n = self.cur
        while n is not None and n.tag != tag:
            n = n.parent
        if n is not None and n.parent is not None:
            self.cur = n.parent

    def handle_data(self, data):
        self.cur.text.append(data)


def parse_layout(path):
    p = P()
    p.feed(open(path, encoding="utf-8").read())
    return p


SIZES = {"INTEGER1x1": 1, "INTEGER1x2": 1, "INTEGER1x3": 1, "INTEGER1x4": 1, "INTEGER1x5": 1,
         "INTEGER1x6": 1, "INTEGER1x7": 1, "INTEGER2x4": 2, "INTEGER2x7": 2, "INTEGER4x4": 4}


def field(row, key):
    m = re.search(key + r":\s*('([^']*)'|[^,}]+)", row)
    if not m:
        return None
    return m.group(2) if m.group(2) is not None else m.group(1).strip()


def digits(v):
    return [(v >> s) & 0xFF for s in (24, 16, 8, 0)]


def lin(v):
    d = digits(v)
    return ((d[0] * 128 + d[1]) * 128 + d[2]) * 128 + d[3]


def unlin(n):
    out = []
    for _ in range(4):
        out.append(n % 128)
        n //= 128
    return list(reversed(out))


def fmt_addr(n):
    return " ".join(f"{b:02X}" for b in unlin(n))


def parse_address_map(path):
    src = Path(path).read_text(encoding="utf-8")
    tables = {}
    for m in re.finditer(r"var (\w+) = \[[^\n]*\n(.*?)\n\t\];", src, re.S):
        rows = [r for r in m.group(2).split("\n") if "addr:" in r]
        tables[m.group(1)] = rows
    leaves, groups = {}, {}
    for name, rows in tables.items():
        if any("child:" in r for r in rows):
            groups[name] = [{"addr": int(field(r, "addr"), 16), "child": field(r, "child"), "name": field(r, "name")} for r in rows]
        else:
            out = []
            for r in rows:
                size = field(r, "size")
                out.append({
                    "off": int(field(r, "addr"), 16),
                    "enc": size if size in SIZES else f"ASCII{size}",
                    "bytes": SIZES.get(size, int(size) if size and size.isdigit() else 1),
                    "ofs": int(field(r, "ofs")),
                    "init": field(r, "init"),
                    "min": int(field(r, "min")),
                    "max": int(field(r, "max")),
                    "id": field(r, "name"),
                })
            leaves[name] = out
    areas = {"SETUP": 0x00000000, "SYSTEM": 0x10000000, "PATCH": 0x20000000, "TEMP": 0x30000000, "COMMAND": 0x7F000000}
    blocks = []
    for area, base in areas.items():
        for g in groups.get(area, []):
            blocks.append({"area": area, "name": g["name"], "table": g["child"],
                           "base": lin(base) + lin(g["addr"]), "params": leaves[g["child"]]})
    return blocks


def control_info(L, items, iid):
    e = L.by_id.get(iid)
    it = items[iid]
    info = {"item": iid}
    if e is None:
        return info
    m = re.search(r"elf-([a-z-]+?)-control", e.attrs.get("class", ""))
    info["kind"] = m.group(1) if m else e.tag
    for k in ("min", "max"):
        if k in e.attrs:
            info[k] = int(e.attrs[k])
    lab = None
    if "description" in e.attrs and e.attrs["description"] in L.by_id:
        lab = L.by_id[e.attrs["description"]].all_text().strip()
    else:
        for suf in ("-dial", "-select-box", "-btn", "-slider", "-knob", "-check-box", "-spinner", "-bar"):
            if iid.endswith(suf) and iid[: -len(suf)] + "-label" in L.by_id:
                lab = L.by_id[iid[: -len(suf)] + "-label"].all_text().strip()
                break
    if lab:
        info["label"] = re.sub(r"\s+", " ", lab)
    box = L.by_id.get(iid + "-box")
    if box is not None:
        opts = [re.sub(r"\s+", " ", c.all_text()).strip() for c in box.children if c.tag == "a"]
        order = it.get("list_order") or list(range(len(opts)))
        info["options"] = {int(v): o for v, o in zip(order, opts)}
    st = L.by_id.get(it.get("stringer", "")) if it.get("stringer") else None
    if st is None:
        cand = re.sub(r"-(dial|slider|knob|spinner|bar)$", "-stringer", iid)
        st = L.by_id.get(cand) if cand != iid else None
    if st is not None:
        f = st.attrs.get("format")
        if f:
            info["format"] = f
        else:
            hidden = [c for c in st.children if c.tag == "div"]
            if hidden:
                info["strings"] = [re.sub(r"\s+", " ", p.all_text()).strip() for p in hidden[0].children if p.tag == "p"]
    page = next((a.id for a in e.ancestors() if a.id and a.id.endswith("-page")), None)
    info["page"] = page
    return info


def build(html):
    html = Path(html)
    blocks = parse_address_map(html / "js/config/address_map.js")
    items = json.loads((html / "export/item.json").read_text(encoding="utf-8"))
    L = parse_layout(html / "export/layout.div")
    by_name = {(b["area"], b["name"]): b for b in blocks}
    for b in blocks:
        for p in b["params"]:
            p["controls"] = []
    unmatched = []
    for iid, it in items.items():
        pid = it.get("pid")
        if not pid or pid.count("%") != 2:
            continue
        area, bname, idx = pid.split("%")
        b = by_name.get((area, bname))
        if b is None or not idx.isdigit():
            continue
        # The pid's last part is the byte offset inside the block, not the row.
        p = next((p for p in b["params"] if lin(p["off"]) == int(idx)), None)
        if p is None:
            unmatched.append(pid)
            continue
        p["controls"].append(control_info(L, items, iid))
    page_order = [e.id for e in L.order if e.id and e.id.endswith("-page")]
    return {"blocks": blocks, "page_order": page_order, "unmatched": unmatched}


COLOURS = ["GREEN", "RED", "ORANGE"]

ROLE = {
    "COM": "Patch name",
    "OTHER": "Chain order, cabinet resonance, master key (harmonist)",
    "COLOR": "Selected colour (variation) of each effect button",
    "AMP": "Amp section",
    "SW": "Effect on/off switches",
    "BOOSTER": "Booster, one block per colour",
    "FX": "MOD/FX type, one block per colour: 1-3 MOD, 4-6 FX",
    "FX_DETAIL": "MOD/FX settings for every type side by side, one block per colour: 1-3 MOD, 4-6 FX",
    "DELAY": "Delay, one block per colour: 1-3 DELAY, 4-6 DELAY 2 (follows the REVERB colour)",
    "REVERB": "Reverb, one block per colour",
    "SOLO_COM": "Solo on/off and level",
    "SOLO_EQ": "Solo EQ",
    "SOLO_DELAY": "Solo delay (write-protected in the current patch, see protocol.md)",
    "CONTOUR_COM": "Contour selector",
    "CONTOUR": "Contour 1-3 settings",
    "PEDALFX_COM": "Pedal FX on/off, position, type",
    "PEDALFX": "Pedal FX settings",
    "EQ_EACH": "EQ 1 / EQ 2: on/off, position, type",
    "EQ_PEQ": "EQ 1 / EQ 2 parametric settings",
    "EQ_GE10": "EQ 1 / EQ 2 graphic (GE-10) settings",
    "NS": "Noise suppressor",
    "SENDRETURN": "Send/return loop",
    "PATCH_KNOB_READONLY": "Panel knob positions (read-only mirror)",
    "PATCH_KNOB_SOLO_DELAY_READONLY": "Solo delay knob position (read-only mirror)",
}

# Which delay / reverb types use which page (from the page names; not checked on the amp).
DELAY_PAGES = {"delay-delay-page": "DIGITAL, STEREO, ANALOG, TAPE ECHO, REVERSE",
               "delay-pan-page": "PAN", "delay-modulate-page": "MODULATE", "delay-sde3000-page": "SDE-3000"}
REVERB_PAGES = {"reverb-reverb-page": "PLATE, ROOM, HALL, MODULATE", "reverb-spring-page": "SPRING"}


def esc(s):
    return str(s).replace("|", "\\|")


def js_format(expr, v):
    e = re.sub(r"\s+", "", expr).replace("<br>", " ")
    sign = "+" if v > 0 else ""
    half = f"{v / 2:.1f}"
    table = {
        "((value>0)?'+':'')+(value/2)+((value&1)?'':'.0')+'dB'": f"{sign}{half} dB",
        "((value>0)?'+':'')+(value/2)+(((value/2)%1)?'':'.0')+'dB'": f"{sign}{half} dB",
        "((value>0)?'+':'')+value+'dB'": f"{sign}{v} dB",
        "((value>0)?'+':'')+value": f"{sign}{v}",
        "(value>0?'+':'')+value": f"{sign}{v}",
        "value+'ms'": f"{v} ms",
        "value+'cm'": f"{v} cm",
        "value+'Hz'": f"{v} Hz",
        "value+'%'": f"{v} %",
        "value": f"{v}",
        "value+1": f"{v + 1}",
        "(value*2)+'%'": f"{v * 2} %",
        "(value/10)+((value%10)?'':'.0')+'s'": f"{v / 10:.1f} s",
        "(value<0)?'OFF':value": "OFF" if v < 0 else f"{v}",
    }
    e = e.replace(" ", "")
    return table.get(e)


class Lists:
    def __init__(self):
        self.lists = []

    def ref(self, strings):
        key = tuple(strings)
        for i, l in enumerate(self.lists):
            if l == key:
                return f"L{i + 1}"
        self.lists.append(key)
        return f"L{len(self.lists)}"


def values(p, lists, skip_options=False):
    """Human description of a parameter's values, from its first useful control."""
    cs = p["controls"]
    for c in cs:
        if c.get("options"):
            if skip_options:
                return "see type list"
            opts = sorted(((int(k), v) for k, v in c["options"].items()))
            return ", ".join(f"{k} {v}" for k, v in opts)
    for c in cs:
        if c.get("strings"):
            s = c["strings"]
            if len(s) <= 6:
                return ", ".join(f"{i} {v}" for i, v in enumerate(s))
            return f"{lists.ref(s)}: {s[0]} … {s[-1]} ({len(s)} steps)"
    for c in cs:
        if c.get("format") and "min" in c:
            a, b = js_format(c["format"], c["min"]), js_format(c["format"], c["max"])
            if a and b:
                return f"{a} … {b}"
    return ""


def labels(p):
    seen = []
    for c in p["controls"]:
        lab = c.get("label") or ""
        page = (c.get("page") or "").removesuffix("-page")
        s = f"{lab} ({page})" if lab else f"({page})"
        if s not in seen:
            seen.append(s)
    return "; ".join(seen[:3]) + (" …" if len(seen) > 3 else "")


def rng(p):
    lo, hi = p["min"], p["max"]
    raw = f"{lo + p['ofs']}–{hi + p['ofs']}"
    return f"{lo}–{hi}" if p["ofs"] == 0 else f"{lo}–{hi} (raw {raw})"


def enc(p):
    e = p["enc"]
    return {"INTEGER1x7": "1", "INTEGER2x4": "2 (nibbles)", "INTEGER2x7": "2", "INTEGER4x4": "4 (nibbles)"}.get(
        e, e.replace("INTEGER1x", "1 (bits ").rstrip() + ")" if e.startswith("INTEGER1x") else e.replace("ASCII", "") + " ASCII")


def short(pid, table):
    prefix = "PRMID_" + table + "_"
    pid = pid.replace("CMDID_", "")
    return pid[len(prefix):] if pid.startswith(prefix) else pid.replace("PRMID_", "")


def verified_map(params_rs):
    out = {}
    src = Path(params_rs).read_text()
    for m in re.finditer(r'p\("([^"]+)", \[0x(\w\w), 0x(\w\w), 0x(\w\w), 0x(\w\w)\]', src):
        out[lin(int("".join(m.group(i) for i in range(2, 6)), 16))] = m.group(1)
    return out


def by_setting(blocks, verified):
    """(table, offset) -> app parameter names, across every instance of the block."""
    out = {}
    for b in blocks:
        for p in b["params"]:
            a = b["base"] + lin(p["off"])
            if a in verified:
                out.setdefault((b["table"], p["off"]), []).append((b["name"], verified[a]))
    return out


def block_table(out, b, lists, verified, *, skip_options=False, with_ui=True, rows=None):
    out.append("| Address | Setting | Bytes | Range | Default | Tone Studio control | Values | App |")
    out.append("|---|---|---|---|---|---|---|---|")
    for p in rows if rows is not None else b["params"]:
        a = b["base"] + lin(p["off"])
        names = verified.get((b["table"], p["off"]), [])
        app = ", ".join(f"`{n}`" if inst == b["name"] else f"`{n}` ({inst})" for inst, n in names[:3])
        if len(names) > 3:
            app += f" +{len(names) - 3} more"
        out.append("| `{}` | {} | {} | {} | {} | {} | {} | {} |".format(
            fmt_addr(a), short(p["id"], b["table"]), enc(p), rng(p), esc(str(p["init"]).strip()),
            esc(labels(p)) if with_ui else "", esc(values(p, lists, skip_options)), app))
    out.append("")


def instances(blocks, table):
    return [b for b in blocks if b["table"] == table]


def checked_settings(m, params_rs):
    """Linear address -> app parameter name, for every setting the app has
    verified on the amp: `params.rs`, plus the generated settings (all
    checked by `katana-cli check-settings`) minus `settings::EXCLUDED`."""
    out = verified_map(params_rs)
    settings_rs = Path(params_rs).with_name("settings.rs").read_text()
    excluded = set(re.findall(r'"([^"]+)"', settings_rs.split("pub const EXCLUDED")[1].split(";")[0]))
    blocks = [b for b in m["blocks"] if b["area"] == "PATCH"]
    for table, names in RUST_TABLES:
        inst = [b for b in blocks if b["table"] == "PATCH_" + table]
        for p in inst[0]["params"]:
            short_id = short(p["id"], "PATCH_" + table)
            if (table in RUST_ONLY and short_id not in RUST_ONLY[table]) or not p["controls"]:
                continue
            if table == "SOLO_DELAY":
                continue  # write-protected: never checked
            key = short_id.lower().replace("_", "-")
            for name, b in zip(names, inst):
                a = b["base"] + lin(p["off"])
                if f"{name}-{key}" not in excluded:
                    out.setdefault(a, f"{name}-{key}")
    return out


def render(m, params_rs, version):
    blocks = m["blocks"]
    checked = checked_settings(m, params_rs)
    verified = by_setting(blocks, checked)
    lists = Lists()
    out = []
    w = out.append
    patch = [b for b in blocks if b["area"] == "PATCH"]
    tables = list(dict.fromkeys(b["table"] for b in patch))
    distinct = sum(len(instances(patch, t)[0]["params"]) for t in tables)
    shown = sum(1 for t in tables for p in instances(patch, t)[0]["params"] if p["controls"])

    w("# KATANA Gen 3 settings reference")
    w("")
    w(f"Every setting BOSS TONE STUDIO for KATANA Gen 3 ({version}) knows for the amp, joined from three "
      "data files in its bundled web UI (source S1 in `protocol.md`):")
    w("")
    w("- `js/config/address_map.js`: address, size, range, default and internal name of every setting;")
    w("- `export/item.json`: which UI control edits which setting (`pid` = area, block, byte offset);")
    w("- `export/layout.div`: each control's page, label, value list and display format.")
    w("")
    w("Generated by `scripts/bts_settings.py` from the user's own installation; only data tables are "
      "read, no logic is copied. **Nothing here is verified on the amp** unless the App column names "
      "one of the app's checked parameters (`katana-proto/src/params.rs`); `protocol.md` stays the "
      "record of verified facts.")
    w("")
    w("Reading the tables:")
    w("")
    w("- Addresses are 4 base-128 digits; a block's settings follow its base address at their byte offset.")
    w("- Range and Default are displayed values. The raw byte is displayed + `ofs`; when they differ, "
      "the raw range is shown too (e.g. `-50–50 (raw 0–100)`).")
    w("- Bytes: `1` = one 7-bit byte; `(nibbles)` = 4 bits per byte, high first (`protocol.md`, value encodings).")
    w("- Tone Studio control: the control's label and its page in Tone Studio's layout.")
    w("- Values: the control's value names, display format or value list. Long lists are in "
      "[Value lists](#value-lists) at the end, referenced as `L1`, `L2`, ….")
    w("")
    w(f"Patch: {len(patch)} blocks, {distinct} distinct settings ({shown} with a Tone Studio control). "
      f"The app has checked {len(verified)} of them on the amp ({len(checked)} addresses, counting every "
      "colour's copy).")
    w("")

    w("## Areas")
    w("")
    w("| Base | Area | What |")
    w("|---|---|---|")
    w("| `00 00 00 00` | SETUP | Current channel selector |")
    w("| `10 00 00 00` | SYSTEM | Global settings, not stored per patch |")
    w("| `20 00 00 00` | PATCH | Current patch (edit buffer); stored channels use the same layout (`protocol.md`) |")
    w("| `30 00 00 00` | TEMP | Panel knob positions while editing |")
    w("| `7F 00 00 00` | COMMAND | Editor commands |")
    w("")

    w("## Patch blocks")
    w("")
    w("| Block | Instances (base addresses) | Settings | What |")
    w("|---|---|---|---|")
    for t in tables:
        inst = instances(patch, t)
        key = t.removeprefix("PATCH_")
        role = ROLE.get(key, "Pedal and footswitch assignments" if key.startswith("ASSIGN") else "")
        bases = ", ".join(f"`{fmt_addr(b['base'])}`" for b in inst)
        w(f"| {key} | {bases} | {len(inst[0]['params'])} | {role} |")
    w("")
    w("Colour instances: block (1) is GREEN, (2) RED, (3) ORANGE (Tone Studio's ids say `grn`, `red`, "
      "`ylw`). FX(1–3) and FX_DETAIL(1–3) belong to MOD, FX(4–6) and FX_DETAIL(4–6) to FX. "
      "DELAY(1–3) belong to DELAY; DELAY(4–6) are DELAY 2, selected by the REVERB colour "
      "(`effect_controller.js`: reverb colour + 3). Tone Studio's controls always name instance (1); "
      "the UI swaps in the selected colour's block.")
    w("")

    def section(title, t, intro=None, **kw):
        inst = instances(patch, t)
        w(f"### {title}")
        w("")
        if intro:
            w(intro)
            w("")
        if len(inst) > 1:
            w("Addresses below are for " + inst[0]["name"] + "; other instances: " +
              ", ".join(f"{b['name']} `{fmt_addr(b['base'])}`" for b in inst[1:]) + ".")
            w("")
        block_table(out, inst[0], lists, verified, **kw)

    w("## Patch: amp, switches, colours")
    w("")
    section("COM", "PATCH_COM")
    section("OTHER", "PATCH_OTHER", "CHAIN is edited by dragging blocks on the chain page; its 9 values are the "
            "chain orders in [Chain orders](#chain-orders).")
    if m.get("chain"):
        out.extend(render_chain(m["chain"]))
    section("COLOR", "PATCH_COLOR", "Values 0–2 = GREEN, RED, ORANGE.")
    section("AMP", "PATCH_AMP", "AMP TYPE 0–5 = ACOUSTIC, CLEAN, PUSHED, CRUNCH, LEAD, BROWN (verified, `params.rs`). "
            "PREAMP_VARIATION and POWERAMP_VARIATION (BLOOM) are check boxes on the panel page.")
    section("SW", "PATCH_SW", "REVERB LAYER MODE decides whether the REVERB button runs DELAY 2, reverb, or both "
            "(`effect_controller.js`).")

    w("## Patch: booster")
    w("")
    section("BOOSTER", "PATCH_BOOSTER")

    # MOD / FX
    fx = instances(patch, "PATCH_FX_DETAIL")[0]
    fxtype = instances(patch, "PATCH_FX")[0]
    # Several controls list the types; the panel's lists abbreviate, so take the longest names.
    order = max((c for c in fxtype["params"][0]["controls"] if c.get("options")),
                key=lambda c: sum(len(v) for v in c["options"].values()))
    type_opts = {int(k): v for k, v in order["options"].items()}
    skip = {"mod-header", "fx-header", "harmonist-content", "harmonist-user", "mod-type", "fx-type"}
    type_pages = [p for p in m["page_order"] if p.startswith("modfx-") and p[6:-5] not in skip]
    w("## Patch: MOD and FX")
    w("")
    w("MOD and FX share one type list and one settings layout. FX(n) holds the type; FX_DETAIL(n) holds the "
      "settings of every type at once, so switching type keeps each type's settings. Type pages appear in "
      "`layout.div` in type-value order (checked against the type list's display order and the "
      "FX_DETAIL names).")
    w("")
    w("Type blocks: " + ", ".join(f"{b['name']} `{fmt_addr(b['base'])}`" for b in instances(patch, "PATCH_FX")) + ".")
    w("Settings blocks: " + ", ".join(f"{b['name']} `{fmt_addr(b['base'])}`" for b in instances(patch, "PATCH_FX_DETAIL")) + ".")
    w("")
    block_table(out, fxtype, lists, verified, skip_options=True)
    w("### Type list")
    w("")
    w("Tone Studio's list order (`list_order`) differs from the values; the menu shows them in the order of the "
      "Menu column.")
    w("")
    menu = {int(v): i + 1 for i, v in enumerate(order["options"].keys())}
    w("| Value | Name | Menu | Tone Studio page | Settings |")
    w("|---|---|---|---|---|")
    by_page = {}
    for p in fx["params"]:
        pages = {c.get("page") for c in p["controls"]}
        for pg in pages:
            key = pg[:-5] if pg else None
            if key and key.startswith("modfx-harmonist"):
                key = "modfx-harmonist"
            by_page.setdefault(key, []).append(p)
    for v, name in sorted(type_opts.items()):
        pg = type_pages[v] if v < len(type_pages) else ""
        w(f"| {v} | {name} | {menu.get(v, '')} | `{pg}` | {len(by_page.get(pg[:-5], []))} |")
    w("")
    for v, name in sorted(type_opts.items()):
        pg = type_pages[v][:-5]
        rows = sorted(by_page.get(pg, []), key=lambda p: lin(p["off"]))
        w(f"### {v} {name}")
        w("")
        extra = [p for b in patch if b["table"] == "PATCH_OTHER" for p in b["params"]
                 if any((c.get("page") or "").startswith(pg) for c in p["controls"])]
        if extra:
            w("Also on this page: " + ", ".join(short(p["id"], "PATCH_OTHER") + " (OTHER block)" for p in extra) + ".")
            w("")
        block_table(out, fx, lists, verified, rows=rows)
    leftover = [p for p in fx["params"] if not p["controls"]]
    if leftover:
        w("FX_DETAIL settings without a control: " + ", ".join(short(p["id"], "PATCH_FX_DETAIL") for p in leftover) + ".")
        w("")

    w("## Patch: delay and reverb")
    w("")
    w("Delay pages by type (from page names): " + "; ".join(f"`{k[:-5]}` = {v}" for k, v in DELAY_PAGES.items()) + ".")
    w("Reverb pages by type: " + "; ".join(f"`{k[:-5]}` = {v}" for k, v in REVERB_PAGES.items()) + ".")
    w("")
    section("DELAY", "PATCH_DELAY", "TIME also appears as the panel's tap-time knob for DELAY and DELAY 2.")
    section("REVERB", "PATCH_REVERB", "LAYER MODE: 0 DELAY (DELAY 2 only), 1 DLY+REV, 2 REVERB.")

    w("## Patch: solo, contour, pedal FX, EQ, noise suppressor, send/return")
    w("")
    for t in ["PATCH_SOLO_COM", "PATCH_SOLO_EQ", "PATCH_SOLO_DELAY", "PATCH_CONTOUR_COM", "PATCH_CONTOUR",
              "PATCH_PEDALFX_COM", "PATCH_PEDALFX", "PATCH_EQ_EACH", "PATCH_EQ_PEQ", "PATCH_EQ_GE10",
              "PATCH_NS", "PATCH_SENDRETURN"]:
        intro = ROLE.get(t.removeprefix("PATCH_"))
        section(t.removeprefix("PATCH_"), t, intro)

    w("## Patch: assignments (expression pedal, footswitches, GA-FC)")
    w("")
    w("Shown for completeness; out of the GUI's scope so far (spec: `ASSIGN_*` excluded). The per-type "
      "ASSIGN_KNOBS entries have no control of their own.")
    w("")
    for t in tables:
        if t.startswith("PATCH_ASSIGN"):
            section(t.removeprefix("PATCH_"), t, with_ui=False)

    w("## Patch: read-only mirrors")
    w("")
    for t in ["PATCH_PATCH_KNOB_READONLY", "PATCH_PATCH_KNOB_SOLO_DELAY_READONLY"]:
        section(t.removeprefix("PATCH_"), t, ROLE.get(t.removeprefix("PATCH_")))

    w("## System")
    w("")
    w("Global settings at `10 00 00 00`; not part of a patch. Out of the GUI's scope so far (spec: SYSTEM deferred).")
    w("")
    system = [b for b in blocks if b["area"] == "SYSTEM"]
    for t in dict.fromkeys(b["table"] for b in system):
        inst = instances(system, t)
        w(f"### {t.removeprefix('SYSTEM_')}")
        w("")
        if len(inst) > 1:
            w("Addresses below are for " + inst[0]["name"] + "; other instances: " +
              ", ".join(f"{b['name']} `{fmt_addr(b['base'])}`" for b in inst[1:]) + ".")
            w("")
        block_table(out, inst[0], lists, verified)

    w("## Setup, temp, command")
    w("")
    for area in ["SETUP", "TEMP", "COMMAND"]:
        for b in [b for b in blocks if b["area"] == area]:
            w(f"### {area} {b['name']}")
            w("")
            block_table(out, b, lists, verified)

    w("## Value lists")
    w("")
    w("Display strings per value (index 0 first), from the controls' hidden string lists.")
    w("")
    for i, l in enumerate(lists.lists):
        w(f"- **L{i + 1}** ({len(l)}): " + ", ".join(f"{j} {esc(s)}" for j, s in enumerate(l)))
    w("")
    return "\n".join(out)


ROOT = Path(__file__).resolve().parent.parent
MAC_HTML = Path("/Applications/BOSS/KATANA Gen 3/BOSS TONE STUDIO for KATANA Gen 3.app/Contents/Resources/html")
WINE_HTML = Path.home() / ".wine/drive_c/users" / Path.home().name / "AppData/Local/Roland/BOSS TONE STUDIO for KATANA Gen 3/html"


# --rust: the tables the editor shows (spec 2026-11-10-katana-full-editor-design.md),
# their instance names, and the only settings kept where a table is shared with
# things out of scope.
RUST_TABLES = [
    ("AMP", ["amp"]),
    ("OTHER", ["other"]),
    ("SW", ["sw"]),
    ("BOOSTER", ["booster1", "booster2", "booster3"]),
    ("FX", ["mod1", "mod2", "mod3", "fx1", "fx2", "fx3"]),
    ("FX_DETAIL", ["mod1", "mod2", "mod3", "fx1", "fx2", "fx3"]),
    ("DELAY", [f"delay{n}" for n in range(1, 7)]),
    ("REVERB", ["reverb1", "reverb2", "reverb3"]),
    ("SOLO_COM", ["solo"]),
    ("SOLO_EQ", ["soloeq"]),
    ("SOLO_DELAY", ["solodelay"]),
    ("CONTOUR_COM", ["contour"]),
    ("CONTOUR", ["contour1", "contour2", "contour3"]),
    ("EQ_EACH", ["eq1", "eq2"]),
    ("EQ_PEQ", ["peq1", "peq2"]),
    ("EQ_GE10", ["geq1", "geq2"]),
]
RUST_ONLY = {"OTHER": {"CHAIN", "MASTER_KEY"}, "SW": {"DELAY2_SW"}}
RUST_ENC = {"INTEGER1x1": "U7", "INTEGER1x2": "U7", "INTEGER1x3": "U7", "INTEGER1x4": "U7", "INTEGER1x5": "U7",
            "INTEGER1x6": "U7", "INTEGER1x7": "U7", "INTEGER4x4": "Nibbles4"}
RUST_FORMATS = {
    "((value>0)?'+':'')+(value/2)+((value&1)?'':'.0')+'dB'": "Format::HalfDb",
    "((value>0)?'+':'')+(value/2)+(((value/2)%1)?'':'.0')+'dB'": "Format::HalfDb",
    "((value>0)?'+':'')+value+'dB'": "Format::SignedDb",
    "((value>0)?'+':'')+value": "Format::Signed",
    "(value>0?'+':'')+value": "Format::Signed",
    "value+'ms'": 'Format::Unit("ms")',
    "value+'cm'": 'Format::Unit("cm")',
    "value+'Hz'": 'Format::Unit("Hz")',
    "value+'%'": 'Format::Unit("%")',
    "value": "Format::Plain",
    "value+1": "Format::PlusOne",
    "(value*2)+'%'": "Format::Percent2",
    "(value/10)+((value%10)?'':'.0')+'s'": "Format::Tenths",
    "(value<0)?'OFF':value": "Format::OffBelowZero",
}
DELAY_TYPES = {"delay-delay": [0, 2, 3, 4, 5], "delay-pan": [1], "delay-modulate": [6], "delay-sde3000": [7]}
REVERB_TYPES = {"reverb-reverb": [0, 1, 2, 4], "reverb-spring": [3]}


def fx_type_pages(m):
    """Tone Studio page name (without -page) of each MOD/FX type, by type value."""
    skip = {"mod-header", "fx-header", "harmonist-content", "harmonist-user", "mod-type", "fx-type"}
    return [p[:-5] for p in m["page_order"] if p.startswith("modfx-") and p[6:-5] not in skip]


def rust_types(table, p, fx_pages):
    pages = {(c.get("page") or "").removesuffix("-page") for c in p["controls"]}
    if table == "FX_DETAIL":
        found = set()
        for pg in pages:
            if pg.startswith("modfx-harmonist"):
                pg = "modfx-harmonist"
            if pg in fx_pages:
                found.add(fx_pages.index(pg))
        return sorted(found)
    by_page = {"DELAY": DELAY_TYPES, "REVERB": REVERB_TYPES}.get(table)
    if not by_page:
        return []
    found = sorted({t for pg in pages for t in by_page.get(pg, [])})
    every = sorted({t for ts in by_page.values() for t in ts})
    return [] if found == every else found


def rust_label(p, key):
    page_first = sorted(p["controls"], key=lambda c: (c.get("page") or "").startswith(("panel", "editor")))
    label = next((c["label"] for c in page_first if c.get("label")), key.replace("-", " ").upper())
    # HARMONIST / PITCH SHIFTER voices: "HR1:HARMONY" -> "HARMONY 1", "HR2: C" -> "USER 2 C".
    m = re.fullmatch(r"(?:HR|PS)(\d):\s*(.+)", label)
    if m:
        voice, rest = m.groups()
        return f"USER {voice} {rest}" if len(rest) <= 2 else f"{rest} {voice}"
    return label


def rust_values(p):
    """(values as [(raw, name)], format) from the setting's controls."""
    lo, hi = p["min"] + p["ofs"], p["max"] + p["ofs"]
    cs = p["controls"]
    for c in cs:
        if c.get("options"):
            best = max((c for c in cs if c.get("options")), key=lambda c: sum(len(v) for v in c["options"].values()))
            vals = sorted((int(k) + p["ofs"], v) for k, v in best["options"].items())
            return [(r, n) for r, n in vals if lo <= r <= hi], "Format::Plain"
    for c in cs:
        if c.get("strings"):
            s = c["strings"]
            if s == ["OFF", "ON"] and (lo, hi) == (0, 1):
                return [(0, "OFF"), (1, "ON")], "Format::Plain"
            if len(s) == hi - lo + 1:
                return list(enumerate(s, lo)), "Format::Names"
            return [(i + p["ofs"], n) for i, n in enumerate(s) if lo <= i + p["ofs"] <= hi], "Format::Names"
    switch = p["id"].endswith("_SW")
    if (switch or any(c.get("kind") == "check-box" for c in cs)) and (lo, hi) == (0, 1):
        return [(0, "OFF"), (1, "ON")], "Format::Plain"
    for c in cs:
        if c.get("format"):
            f = RUST_FORMATS.get(re.sub(r"\s+", "", c["format"]).replace("<br>", ""))
            if f:
                return None, f
    return None, "Format::Plain"


def rust_str(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def render_rust(m):
    blocks = [b for b in m["blocks"] if b["area"] == "PATCH"]
    fx_pages = fx_type_pages(m)
    lists = {}
    out_blocks = []
    for table, names in RUST_TABLES:
        inst = [b for b in blocks if b["table"] == "PATCH_" + table]
        if len(inst) != len(names):
            sys.exit(f"{table}: {len(inst)} instances, expected {len(names)}")
        rows = []
        for p in inst[0]["params"]:
            short_id = short(p["id"], "PATCH_" + table)
            if table in RUST_ONLY and short_id not in RUST_ONLY[table]:
                continue
            if not p["controls"]:
                continue
            if p["enc"] not in RUST_ENC:
                sys.exit(f"{table} {short_id}: encoding {p['enc']} not supported")
            key = short_id.lower().replace("_", "-")
            values, fmt = rust_values(p)
            vref = "None"
            if values:
                body = ", ".join(f"({r}, {rust_str(n)})" for r, n in values)
                name = lists.setdefault(body, f"V{len(lists)}")
                vref = f"Some({name})"
            types = rust_types(table, p, fx_pages)
            if table == "OTHER" and short_id == "MASTER_KEY":
                types = [12]
            rows.append(
                f"        GenSetting {{ key: {rust_str(key)}, label: {rust_str(rust_label(p, key))}, "
                f"offset: {lin(p['off'])}, encoding: Encoding::{RUST_ENC[p['enc']]}, "
                f"min: {p['min'] + p['ofs']}, max: {p['max'] + p['ofs']}, ofs: {p['ofs']}, values: {vref}, "
                f"format: {fmt}, types: &[{', '.join(map(str, types))}] }},")
        bases = ", ".join(f"({rust_str(n)}, {b['base']})" for n, b in zip(names, inst))
        out_blocks.append(f"    GenBlock {{\n        table: {rust_str(table)},\n        inst: &[{bases}],\n"
                          f"        settings: &[\n" + "\n".join("    " + r for r in rows) + "\n        ],\n    },")
    out = ["// @generated by scripts/bts_settings.py --rust from BOSS TONE STUDIO's tables; do not edit.",
           "", "use crate::params::{Encoding, Format};", "",
           "/// One setting of a block; `offset` is the linear byte offset in the block.",
           "#[derive(Debug)]",
           "pub struct GenSetting {",
           "    pub key: &'static str,",
           "    pub label: &'static str,",
           "    pub offset: u32,",
           "    pub encoding: Encoding,",
           "    /// Raw range.",
           "    pub min: u32,",
           "    pub max: u32,",
           "    /// Shown value = raw − `ofs`.",
           "    pub ofs: i32,",
           "    pub values: Option<&'static [(u32, &'static str)]>,",
           "    pub format: Format,",
           "    /// The block types that use this setting; empty = every type.",
           "    pub types: &'static [u32],",
           "}", "",
           "/// A block layout and its instances (name, linear base address).",
           "#[derive(Debug)]",
           "pub struct GenBlock {",
           "    pub table: &'static str,",
           "    pub inst: &'static [(&'static str, u32)],",
           "    pub settings: &'static [GenSetting],",
           "}", ""]
    for body, name in lists.items():
        out.append(f"const {name}: &[(u32, &str)] = &[{body}];")
    out += ["", "pub const BLOCKS: &[GenBlock] = &["] + out_blocks + ["];", ""]
    return "\n".join(out)


CHAIN_NAMES = {"pdl": "PEDAL FX", "bst": "BOOSTER", "amp": "AMP", "mod": "MOD", "fx": "FX", "fv": "FOOT VOLUME",
               "sr": "SEND/RETURN", "dly1": "DELAY", "dly2": "DELAY 2", "rev": "REVERB", "eq": "EQ 1", "eq2": "EQ 2"}


def parse_chain(html):
    """Chain orders, allowed moves and EQ placement from js/businesslogic/chain/chain_config.js."""
    src = (html / "js/businesslogic/chain/chain_config.js").read_text(encoding="utf-8")
    flow = src[src.index("CHAIN_BASE_FLOW"):src.index("CHAIN_BASE_TRANSITION")]
    base = flow[flow.index("pdlVal: 0, srVal: 0"):flow.index("pdlVal: 0, srVal: 1")]
    orders = [re.findall(r"'(\w+)'", row) for row in re.findall(r"\[('[^\]]*)\]", base)]
    trans = src[src.index("CHAIN_BASE_TRANSITION"):src.index("CHAIN_EQ_FLOW")]
    moves = {}
    for v, body in re.findall(r"\{patchVal:\s*(\d+),([^}]*)\}", trans):
        moves[int(v)] = {k: [int(n) for n in re.findall(r"\d+", ns)]
                         for k, ns in re.findall(r"'(\w+)':\s*\[([^\]]*)\]", body)}
    eq = [(int(a), int(b), re.findall(r"'(\w+)'", l))
          for a, b, l in re.findall(r"eqPos1:\s*(\d+),\s*eqPos2:\s*(\d+),\s*list:\s*\[([^\]]*)\]", src)]
    return {"orders": orders, "moves": moves, "eq": eq}


def render_chain(chain):
    out = []
    w = out.append
    w("### Chain orders")
    w("")
    w("From Tone Studio's chain page (`js/businesslogic/chain/chain_config.js`). CHAIN (`20 00 02 00`) "
      "picks one of 9 orders. Shown with PEDAL FX at INPUT and SEND/RETURN at its first position; "
      "PEDAL FX POSITION POST AMP moves it to just after the AMP, and the SEND/RETURN position moves "
      "that block (after the reverb, or just after the AMP).")
    w("")
    w("| CHAIN | Order | Drag moves (block → CHAIN value) |")
    w("|---|---|---|")
    for v, order in enumerate(chain["orders"]):
        names = " → ".join(f"**{CHAIN_NAMES[b]}**" if b == "amp" else CHAIN_NAMES.get(b, b) for b in order)
        mv = "; ".join(f"{CHAIN_NAMES[k]} → {', '.join(map(str, ns))}" for k, ns in chain["moves"].get(v, {}).items())
        w(f"| {v} | {names} | {mv} |")
    w("")
    w("A drag move lists the orders a block can be dropped into from that order; every other drop is refused.")
    w("")
    w("EQ 1 and EQ 2 sit next to the AMP, by their POSITION settings (EQ_EACH, 0 AMP IN, 1 AMP OUT):")
    w("")
    w("| EQ 1 | EQ 2 | Around the AMP |")
    w("|---|---|---|")
    pos = {0: "AMP IN", 1: "AMP OUT"}
    for a, b, l in chain["eq"]:
        w(f"| {pos[a]} | {pos[b]} | " + " → ".join(CHAIN_NAMES[x] for x in l) + " |")
    w("")
    return out


def app_version(html):
    plist = html.parent.parent / "Info.plist"
    if plist.exists():
        import plistlib
        info = plistlib.loads(plist.read_bytes())
        return f"{info.get('CFBundleShortVersionString')} build {info.get('CFBundleVersion')}, macOS"
    return "Windows build under Wine"


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--html", type=Path, help="Tone Studio's html folder")
    ap.add_argument("--out", type=Path, default=ROOT / "docs/settings.md")
    ap.add_argument("--rust", type=Path, help="write the editor's settings table (Rust) here instead")
    args = ap.parse_args()
    html = args.html or next((p for p in (MAC_HTML, WINE_HTML) if p.exists()), None)
    if html is None or not (html / "js/config/address_map.js").exists():
        sys.exit("BOSS TONE STUDIO's html folder not found; pass --html")
    m = build(html)
    if args.rust:
        args.rust.write_text(render_rust(m), encoding="utf-8")
        print(f"wrote {args.rust}")
        return
    m["chain"] = parse_chain(html)
    text = render(m, ROOT / "katana-proto/src/params.rs", app_version(html))
    args.out.write_text(text, encoding="utf-8")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
