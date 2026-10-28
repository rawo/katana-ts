#!/usr/bin/env bash
# Build, test and run katana-ts in a Linux VM (VirtualBox) from the Mac.
# The source is copied to the VM with rsync; the GUI starts on the VM's desktop.
# Usage: scripts/vm.sh sync|test|build|run|stop|log|shot [file]|ssh [cmd…]
# Settings (env): VM_HOST (vboxuser@127.0.0.1), VM_PORT (2222), VM_NAME (ubuntu), VM_DIR (katana-ts)
set -euo pipefail

host=${VM_HOST:-vboxuser@127.0.0.1}
port=${VM_PORT:-2222}
name=${VM_NAME:-ubuntu}
dir=${VM_DIR:-katana-ts}
root=$(cd "$(dirname "$0")/.." && pwd)

remote() { ssh -p "$port" -o BatchMode=yes "$host" "$@"; }
in_dir() { remote ". ~/.cargo/env && cd $dir && $*"; }

sync() {
  rsync -az --delete -e "ssh -p $port -o BatchMode=yes" \
    --exclude /target --exclude /.git --exclude /.idea --exclude /backups --exclude /.superpowers \
    "$root/" "$host:$dir/"
}

# The desktop session's environment, so the app opens on the VM's screen.
# Software GL: with VirtualBox's VMSVGA adapter the window otherwise stays see-through.
desktop_env='uid=$(id -u); export XDG_RUNTIME_DIR=/run/user/$uid WAYLAND_DISPLAY=wayland-0 LIBGL_ALWAYS_SOFTWARE=1 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$uid/bus'

case ${1:-} in
  sync) sync ;;
  test) sync && in_dir cargo test --workspace ;;
  build) sync && in_dir cargo build -p katana-gui --release ;;
  run)
    sync && in_dir cargo build -p katana-gui --release
    remote "$desktop_env; pkill -x katana-gui || true; cd $dir && setsid -f target/release/katana-gui </dev/null >~/katana-gui.log 2>&1"
    echo "started katana-gui on the VM desktop (log: scripts/vm.sh log)"
    ;;
  stop) remote 'pkill -x katana-gui || true' ;;
  log) remote 'tail -n 50 ~/katana-gui.log' ;;
  shot)
    out=${2:-$root/target/vm-shot.png}
    mkdir -p "$(dirname "$out")"
    VBoxManage controlvm "$name" screenshotpng "$out"
    echo "wrote $out"
    ;;
  ssh) shift; remote "$@" ;;
  *) sed -n 2,5p "$0" >&2; exit 1 ;;
esac
