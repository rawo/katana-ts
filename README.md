# katana-ts

Tools for the BOSS **KATANA-100 Gen 3** amp over USB, for Linux (where BOSS TONE
STUDIO doesn't run):

| Crate | What it is |
|---|---|
| `katana-proto` | Protocol library: Roland SysEx (RQ1/DT1), patch read/write, verified parameters. |
| `katana-cli` | Command-line tool: store patches into channels, dump channels to `.syx`/`.tsl`, set parameters, watch the amp, analyse captures. |
| `katana-gui` | Desktop editor (egui). Edits `.syx` files offline; with the amp connected, plays and stores them on it. |

Linux is the only target. The code also builds on macOS for development.

## Requirements

- Rust (stable, edition 2021) with `cargo`.
- Linux build packages: `pkg-config` and the ALSA headers (`libasound2-dev` on
  Debian/Ubuntu, `alsa-lib-devel` on Fedora). `midir` needs them.
- For the GUI's file dialogs on Linux, the `xdg-desktop-portal` service (standard on
  GNOME and KDE).
- The amp connected by USB. It shows up as the class-compliant MIDI port `KATANA3`.
  Close the official app (or anything else using the port) first.

## Build

```bash
cargo build                         # whole workspace, debug
cargo build --release               # whole workspace, optimised
cargo build -p katana-cli           # one crate
cargo build -p katana-gui --release
```

Binaries land in `target/debug/` or `target/release/` (`katana-cli`, `katana-gui`).

## Run

```bash
cargo run -p katana-gui                    # GUI
cargo run -p katana-gui --release
cargo run -p katana-cli -- <command> …     # CLI, e.g. cargo run -p katana-cli -- dump now.syx
./target/release/katana-cli <command> …    # after a release build
```

## Test

```bash
cargo test --workspace                       # everything that needs no hardware
cargo test -p katana-proto                   # protocol library only
cargo test -p katana-gui                     # GUI (headless, egui_kittest)
cargo test -p katana-gui --test files        # one test file
cargo test -p katana-proto layout            # tests whose name contains "layout"
```

Hardware tests talk to a real amp. Run them by hand, with the amp connected and
the official app closed:

```bash
cargo test -p katana-proto --features hardware-tests
```

- `amp_answers_identity_request`: the amp answers a MIDI Identity Request.
- `hardware_patch_round_trip_is_byte_identical`: writes a different stored patch
  into the current patch, reads it back, then restores the original and reads that
  back.

`katana-proto` features: `midi` (default; the `midir` transport), `testing` (exports
the `FakeAmp` test double), `hardware-tests` (the tests above).

## Testing on Linux from a Mac (VM)

`scripts/vm.sh` builds, tests and runs the workspace in a Linux VM over SSH, so the
GUI can be checked on Linux without switching machines. It copies the source into
the VM with `rsync` (leaving out `target/`, `.git/`, `.idea/`, `backups/`,
`.superpowers/`) and starts the GUI on the VM's Wayland desktop.

```bash
scripts/vm.sh sync          # copy the source to ~/katana-ts in the VM
scripts/vm.sh test          # sync, then cargo test --workspace in the VM
scripts/vm.sh build         # sync, then build katana-gui --release
scripts/vm.sh run           # sync, build, (re)start katana-gui on the VM desktop
scripts/vm.sh stop          # stop katana-gui
scripts/vm.sh log           # last 50 lines of the GUI's output
scripts/vm.sh shot [FILE]   # screenshot of the VM screen (default target/vm-shot.png)
scripts/vm.sh ssh [CMD…]    # run a command in the VM (no CMD: interactive shell)
```

| Variable | Default | Meaning |
|---|---|---|
| `VM_HOST` | `vboxuser@127.0.0.1` | SSH user and host. |
| `VM_PORT` | `2222` | SSH port. |
| `VM_NAME` | `ubuntu` | VirtualBox VM name (for `shot`). |
| `VM_DIR` | `katana-ts` | Folder in the VM user's home. |

One-time setup (VirtualBox, Ubuntu with a GNOME Wayland desktop):

```bash
# in the VM
sudo apt install -y openssh-server pkg-config libasound2-dev build-essential rsync
wget -qO- https://sh.rustup.rs | sh -s -- -y --profile minimal
gsettings set org.gnome.desktop.session idle-delay 0   # a locked screen screenshots black
# on the Mac
VBoxManage controlvm ubuntu natpf1 "ssh,tcp,127.0.0.1,2222,,22"
ssh-copy-id -p 2222 vboxuser@127.0.0.1
```

The VM can't reach the amp (no USB passthrough by default), so amp work still needs
the Linux machine. A VM on an ARM Mac builds arm64 binaries; build on the target
machine itself.

## Lint and format

```bash
cargo fmt                              # rustfmt.toml: max_width 120
cargo fmt --check
cargo clippy --workspace --all-targets
```

## GUI

`katana-gui` opens where you left it: the patch you were editing when you last
quit, with its unsaved changes (see Auto-save below). The first time, it opens a
default preset: everything off or 0, amp type CLEAN.

- **Top bar, first row:** connection state and channel tabs (PANEL, A1–A4, B1–B4)
  on the left; the patch name (its file name in the library), an "unsaved" marker
  when it has changed, and Live edit on the right.
- **Top bar, second row:** the Editor and Library tabs on the left; the actions on
  the right.
  - **Import…** copies a `.syx` patch into My patches, or a Tone Exchange `.tsl`
    set into Downloaded sets (the file is kept as it is). A name that is taken gets
    a number: `Iron Man (2)`.
  - **Export .syx…** writes a copy of the patch anywhere, e.g. to share it (the
    same format as `katana-cli dump`; `.syx` is added if you leave it off).
  - **Quick save** saves to the Quick saves group as
    `quick-save_2026-10-28_15-04-12.syx`. If the patch came from a quick save, it
    overwrites that one; it never overwrites a named patch.
  - **Save** overwrites the file the patch came from (greyed out when that patch is
    read-only). With no file yet, it works like Save as….
  - **Save as…** asks for a name and saves a new patch in My patches; the name is
    also written into the patch (cut to 16 characters).
  - Live edit, Load to amp and Store to channel… are greyed out until amp control
    (stage 1) exists.
  - Opening a patch, or closing the window, with unsaved changes asks first; Escape
    or a click outside the question cancels. Every save writes a temporary file
    first, so a failed save leaves the old file as it was.
- **Window:** at least 1200×720, so the whole signal chain fits.
- **Editor tab:**
  - The signal chain IN → BOOSTER → MOD → AMP → FX → DELAY → REVERB → OUT. Each
    block shows its on/off switch (click to toggle) and, where known, its type and
    variation lights (click a light to pick GREEN, RED or ORANGE).
  - Click a block to see its parameters in the panel below.
  - The amp knobs (GAIN … PRESENCE) are always visible.
- **Knobs:** drag up or down (200 px sweeps the full range); the mouse wheel moves
  one step; double-click to type a value, Enter to accept, Escape to cancel.
  While you scroll the page, the wheel keeps scrolling past knobs; rest on a knob
  for half a second and the wheel turns it. Switches are toggles; named values are
  dropdowns. A switch byte that is neither 0 nor 1 shows as `? (n)`.
- **Library tab:** **Search** (shows names containing any of the words), **Sort**
  (newest or oldest first, name A–Z or Z–A) and **Import…** on top, then two lists:
  - **My patches** — your patches, with the **Quick saves** group below them. Click
    a name to open it in the editor. **read-only** locks a patch (Save and Delete
    are then refused); **Delete** asks first.
  - **Downloaded sets** — imported Tone Exchange sets, by set name. Open a set to
    see its patches; **Open** saves that patch as a new file in My patches (named
    after the patch, numbered if taken) and loads it into the editor. **Delete set**
    asks first and keeps the patches you already took from it. A patch that can't
    be mapped completely onto a KATANA Gen 3 patch is refused with a message.
- **Auto-save:** about every 5 minutes while the patch keeps changing, after every
  Save, Save as… and Quick save, and when the window closes, the patch is saved to
  a hidden folder (the last 10 are kept). The next start reopens it, together with
  the file it came from. If you choose Discard when closing, the next start opens
  that file as it was saved.
- **Status line:** amp state, time of the last auto-save, last error (including
  library files that couldn't be read and were skipped), and the **Theme** picker
  (Dark or Light).

Only parameters verified on the amp appear (see the table below). Blocks with none
yet say so.

### With the amp

The app finds the amp by itself (USB; the official app should be closed). The dot
in the top bar says whether it is connected.

- **Nothing is stored into a channel without a click on Store.** Editing, Open, Live
  edit, Save and Load to amp only change what the amp plays right now.
- **Live edit** is off at every start. With it on, the app and the amp follow each
  other: edits in the app are heard at once, panel changes and channel switches on the
  amp show up in the app, and clicking a channel tab switches the amp. Opening a
  preset plays it, without storing it. Live edit keeps its position if the amp is
  unplugged and comes back.
- **Channel tabs** with Live edit off show what a channel holds, without switching
  the amp. Without the amp they show the last copy seen, kept in `channels/` in the
  library folder.
- **Load to amp** plays the editor's preset on the channel the amp is on; it doesn't
  switch channels or store anything.
- **Store to channel…** asks first, showing what the channel holds now. It backs up
  that channel and the current sound to `backups/` in the library folder, the same
  place the CLI uses, stores, reads the channel back, and leaves the amp on that
  channel. A channel keeps its own solo delay.
- Before the amp's settings replace the editor, unsaved changes go into a quick save.
  When they replace a library preset, the top bar shows "Saving will overwrite library
  preset ...": Save would write them into that file. Save as… keeps both.

### Library folder

The library lives in `~/.local/share/katana-ts/` (on macOS, during development:
`~/Library/Application Support/katana-ts/`):

| Path | Holds |
|---|---|
| `patches/` | My patches (`.syx`, the `katana-cli dump` format) |
| `quick-saves/` | Quick saves |
| `sets/` | Downloaded Tone Exchange sets (`.tsl`, unchanged) |
| `autosave/` | The last 10 auto-saves and `state.json` |
| `index.json` | Read-only flags and which set each picked patch came from |

Library patches are plain `.syx` files, so the CLI works on them directly. A typical
offline round trip:

```bash
katana-cli dump mine.syx       # amp → file
cargo run -p katana-gui        # Import… mine.syx, edit, Save
katana-cli load ~/.local/share/katana-ts/patches/mine.syx   # file → amp (current patch)
```

## CLI

```
katana-cli [--port <PORT>] [--model-id <HEX>] <COMMAND>
```

Global options (they work with every command):

| Option | Default | Meaning |
|---|---|---|
| `--port <PORT>` | `KATANA3` | Use the first MIDI port whose name contains this text. |
| `--model-id <HEX>` | `01 05 07` | Model ID used to decode messages (`extract`/`diff`/`monitor`), e.g. `--model-id 00000033`. Spaces and `:` are allowed. |
| `-h`, `--help` | | Help; works per command too (`katana-cli dump --help`). |
| `-V`, `--version` | | Version. |

### Commands

| Command | Needs amp | What it does |
|---|---|---|
| `ports` | no | Lists MIDI ports. |
| `identity` | yes | Sends a MIDI Identity Request and prints the reply. |
| `monitor [--editor]` | yes | Prints SysEx sent by the amp until Ctrl-C. It only sees amp → host traffic; use a usbmon capture for the other direction. `--editor` turns editor mode on while listening (the amp then reports channel changes and more by itself) and off again on Ctrl-C. |
| `extract <FILE>` | no | Prints the SysEx messages in a capture export (`.tsv`) or a `.syx` file, decoded. |
| `diff <A> <B>` | no | Shows the bytes that differ between the DT1 data in two files (`.syx` or `.tsv`). |
| `dump <OUT>` | yes | Reads the amp's current patch into a `.syx` file. |
| `dump <CHANNEL> <FILE> [--yes]` | yes | Reads stored channel `A1`–`A4`, `B1`–`B4` (any case) into a whole file; `.syx` or `.tsl` (a one-patch Tone Exchange set) is picked by the extension. Asks before overwriting an existing file; `--yes` doesn't ask. The amp doesn't switch channel. |
| `store <FILE> <CHANNEL> [--patch <N>] [--yes]` | yes | Stores a `.syx` patch, or patch `N` of a `.tsl` set (`--patch` is required for a set, even with one patch), into a channel. First backs up the channel and the amp's current sound to the backup folder (below) and prints both paths, then asks `A3 currently '…' → replace with '…'? [y/N]`; `--yes` doesn't ask. Reads the channel back to check, and leaves the amp on that channel. Prints "solo delay not stored" when the amp can't take it (the channel keeps its own). |
| `tsl-list <FILE>` | no | Lists a `.tsl` set's patches, numbered for `store --patch`. |
| `load <FILE>` | yes | Writes a `.syx` patch (from `dump`) into the amp's current patch. The whole file is checked first; nothing is sent if it isn't a complete Gen 3 patch. SOLO_DELAY settings can't be written (the amp ignores them). |
| `set <NAME> <VALUE>` | yes | Sets one parameter of the current patch. `VALUE` is a number or a value name (case-insensitive), e.g. `set amp-type crunch`. Out-of-range values are rejected before anything is sent. |
| `params` | yes | Lists the known parameters with range and current value. |
| `verify [--slot <N>]` | yes | Proves writes land: writes a different stored patch into the current patch, reads it back, restores the original and reads that back. `--slot N` (1–10) picks stored patch PATCH(N); by default it uses the first one that differs from the current patch. |
| `watch [--map <PATH>] [--interval-ms <MS>]` | yes | Polls the current patch and prints each parameter that changes, until Ctrl-C. Turn knobs on the amp to see their addresses. |

`watch` options:

| Option | Default | Meaning |
|---|---|---|
| `--map <PATH>` | `$WINEPREFIX` (or `~/.wine`)`/drive_c/users/$USER/AppData/Local/Roland/BOSS TONE STUDIO for KATANA Gen 3/html/js/config/address_map.js` | The official app's address table, used to name the changed bytes. Without it, only addresses are printed. |
| `--interval-ms <MS>` | `300` | Milliseconds between reads. |

Questions default to No. Without a terminal (e.g. in a script) `dump` and
`store` refuse to overwrite unless given `--yes`. Ctrl-C during `store`'s talk
with the amp waits until it is finished, so the amp is never left in editor
mode.

Backups made by `store` go to `~/.local/share/katana-ts/backups/`, named
`<date>_<time>_<channel or current>_<patch name>.syx`; put one back with
`katana-cli store <backup>.syx <channel>`.

Channels and stored slots: PANEL = PATCH(1), A1–A4 = PATCH(2)–(5), B1–B4 =
PATCH(6)–(9), PATCH(10) = the app's preview buffer (see `docs/protocol.md`).

### Examples

```bash
katana-cli ports
katana-cli identity
katana-cli dump backup.syx
katana-cli load backup.syx
katana-cli tsl-list "Black Sabbath.tsl"
katana-cli store "Black Sabbath.tsl" A3 --patch 4
katana-cli store mine.syx B4
katana-cli dump A3 crunch.tsl
katana-cli dump A3 crunch.syx
katana-cli params
katana-cli set gain 60
katana-cli set booster-sw on
katana-cli set booster-color orange
katana-cli set delay2-time 450
katana-cli diff before.syx after.syx
katana-cli verify --slot 5
katana-cli watch --interval-ms 200
katana-cli --port "KATANA3 MIDI 1" dump now.syx
```

### Parameters (`set` / `params`)

All verified on the amp (evidence in `docs/protocol.md`). They are current-patch
parameters. Each is one byte (0–127), except `delay2-time`, which is four 4-bit
nibbles.

| Name | Range | Values |
|---|---|---|
| `gain` | 0–100 | |
| `volume` | 0–100 | |
| `bass` | 0–100 | |
| `middle` | 0–100 | |
| `treble` | 0–100 | |
| `presence` | 0–100 | |
| `amp-type` | 0–5 | `acoustic`, `clean`, `pushed`, `crunch`, `lead`, `brown` |
| `booster-sw` | 0–1 | `off`, `on` |
| `mod-sw` | 0–1 | `off`, `on` |
| `fx-sw` | 0–1 | `off`, `on` |
| `delay-sw` | 0–1 | `off`, `on` |
| `reverb-sw` | 0–1 | `off`, `on` |
| `solo-sw` | 0–1 | `off`, `on` |
| `booster-color` | 0–2 | `green`, `red`, `orange` (selects booster 1/2/3) |
| `cab-resonance` | 0–2 | `vintage`, `modern`, `deep` |
| `contour` | 0–3 | `off`, `1`, `2`, `3` |
| `booster1-drive` | 0–120 | drive, green variation |
| `booster2-drive` | 0–120 | drive, red variation |
| `booster3-drive` | 0–120 | drive, orange variation |
| `delay2-time` | 1–2000 | ms |
| `delay2-feedback` | 0–100 | |
| `delay2-level` | 0–120 | effect level |
| `reverb1-level` | 0–100 | effect level |

Amp quirk: changing a variation colour makes the amp re-derive that effect's on/off
from the physical knob a moment later. Send the colour first, then on/off.

## Capturing USB traffic (protocol work)

Linux only. Needs `tshark`, `lsusb`, and permission to capture on `usbmon*`
(e.g. membership of the `wireshark` group). Details: `captures/README.md`.

```bash
captures/tools/capture.sh NN-short-description     # capture until Ctrl-C (loads usbmon with sudo)
captures/tools/export.sh captures/NN-short-description.pcapng   # → .pcapng.tsv
katana-cli extract captures/NN-short-description.pcapng.tsv
katana-cli diff a.tsv b.tsv
```

## Docs

- `docs/protocol.md`: the verified protocol facts and amp quirks.
- `docs/superpowers/specs/`: design specs (protocol, GUI).
- `docs/superpowers/plans/`: implementation plans.
