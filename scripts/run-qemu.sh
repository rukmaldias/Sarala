#!/bin/sh
# Boot the stage-0 image under emulation.
#
# Runs on the macOS host, NOT nested inside the build VM — see
# docs/build-environment.md. Emulated aarch64 under TCG is slow and that is
# fine: this target boots a shell and never compiles anything.
#
# Exit QEMU with Ctrl-A then X.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUT="$ROOT/out"

KERNEL=${KERNEL:-$OUT/Image}
INITRAMFS=${INITRAMFS:-$OUT/initramfs.cpio.gz}

for f in "$KERNEL" "$INITRAMFS"; do
	[ -f "$f" ] || {
		echo "missing $f" >&2
		exit 1
	}
done

exec qemu-system-aarch64 \
	-M virt \
	-cpu cortex-a57 \
	-smp 2 \
	-m 1024 \
	-nographic \
	-kernel "$KERNEL" \
	-initrd "$INITRAMFS" \
	-append "console=ttyAMA0 rdinit=/init panic=10"
