# Captures

Raw USB traffic between the official BOSS TONE STUDIO (under Wine) and the
amp, kept as evidence for every entry in `docs/protocol.md`.

## One-time setup

- `tshark` installed, and your user able to capture on `usbmon*`
  (e.g. member of the `wireshark` group; log out/in after adding).

## Session procedure (one change per capture)

1. Close every other program using the amp.
2. `captures/tools/capture.sh NN-short-description`
3. In the official app, do **exactly one** action.
4. Ctrl-C to stop.
5. `captures/tools/export.sh captures/NN-short-description.pcapng`
6. Inspect: `katana-cli extract captures/NN-….pcapng.tsv`;
   compare two states: `katana-cli diff a.tsv b.tsv`.

## Naming

`NN-short-description.{pcapng,device,pcapng.tsv}` — `NN` is a sequence
number (`01`, `02`, …; per-parameter captures use `1NN`). Commit all three
files; the `.tsv` is what the golden tests read.
