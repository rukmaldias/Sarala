#!/bin/sh
# Assemble the stage-0 initramfs: PID 1, a busybox shell, and nothing else.
#
# Run in the Debian build VM. Produces out/initramfs.cpio.gz.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUT="$ROOT/out"
STAGE="$OUT/initramfs"
TARGET=aarch64-unknown-linux-musl

# Static aarch64 busybox. Cross-compile it or drop a prebuilt binary here.
BUSYBOX=${BUSYBOX:-$OUT/busybox}

[ -x "$BUSYBOX" ] || {
	echo "no static busybox at $BUSYBOX (override with BUSYBOX=)" >&2
	exit 1
}

cargo build --release --target "$TARGET"

rm -rf "$STAGE"
mkdir -p "$STAGE"/bin "$STAGE"/sbin "$STAGE"/proc "$STAGE"/sys "$STAGE"/dev "$STAGE"/run "$STAGE"/root

install -m 0755 "$ROOT/target/$TARGET/release/init" "$STAGE/init"
install -m 0755 "$ROOT/target/$TARGET/release/bootctrl" "$STAGE/sbin/bootctrl"
install -m 0755 "$BUSYBOX" "$STAGE/bin/busybox"

# Busybox dispatches on argv[0], so every applet is a link to the one binary.
for applet in sh ls cat mount umount dmesg ps mkdir echo sleep; do
	ln -sf busybox "$STAGE/bin/$applet"
done

# The kernel opens /dev/console as PID 1's stdin/stdout/stderr *before* exec, but
# only if the node already exists in the initramfs — devtmpfs is not mounted yet.
# Without it PID 1 starts with fds 0/1/2 closed: every early message and any
# panic is lost, and reopened fds can silently land on 0/1/2. Ship the node (plus
# /dev/null). mknod needs no real root under fakeroot, which also records the
# cpio entries as owned by root:root.
fakeroot sh -c "
	mknod -m 0600 '$STAGE/dev/console' c 5 1
	mknod -m 0666 '$STAGE/dev/null'    c 1 3
	cd '$STAGE' && find . | cpio -o -H newc --quiet
" | gzip -9 >"$OUT/initramfs.cpio.gz"

echo "out/initramfs.cpio.gz: $(du -h "$OUT/initramfs.cpio.gz" | cut -f1)"
