#!/usr/bin/env bash
# Capture all USB traffic on the KATANA's bus until Ctrl-C.
# Usage: captures/tools/capture.sh <NN-short-description>
set -euo pipefail

name=${1:?usage: capture.sh <NN-short-description>}
here=$(cd "$(dirname "$0")/.." && pwd)
out="$here/$name.pcapng"

line=$(lsusb -d 0582:02f0) || { echo "KATANA3 (0582:02f0) not found on USB" >&2; exit 1; }
bus=$((10#$(awk '{print $2}' <<<"$line")))
dev=$((10#$(awk '{print $4}' <<<"$line" | tr -d :)))

sudo modprobe usbmon
echo "$bus $dev" > "$here/$name.device"
echo "Capturing usbmon$bus (device $dev) to $out — press Ctrl-C to stop."
tshark -i "usbmon$bus" -w "$out" || true
