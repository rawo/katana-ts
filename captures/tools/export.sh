#!/usr/bin/env bash
# Export the KATANA's USB-MIDI payloads from a capture as
# "<direction>\t<hex>" lines (0 = host->amp, 1 = amp->host).
# Usage: captures/tools/export.sh <capture.pcapng>   -> writes <capture.pcapng>.tsv
set -euo pipefail

pcap=${1:?usage: export.sh <capture.pcapng>}
devfile="${pcap%.pcapng}.device"
[[ -f $devfile ]] || { echo "missing $devfile (written by capture.sh)" >&2; exit 1; }
read -r bus dev < "$devfile"

tshark -r "$pcap" \
  -Y "usb.bus_id == $bus && usb.device_address == $dev && usb.transfer_type == 0x03 && usb.capdata" \
  -T fields -e usb.endpoint_address.direction -e usb.capdata \
  > "$pcap.tsv"
echo "wrote $pcap.tsv ($(wc -l < "$pcap.tsv") transfers)"
