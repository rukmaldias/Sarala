#!/bin/sh
# Manage the x86_64 Debian build VM.
#
# Runs on the macOS host. This VM is where everything is compiled; the
# aarch64 target in run-qemu.sh only ever boots. See docs/build-environment.md
# for why the build host is x86_64 and not aarch64.
#
#   vm.sh fetch     download and verify the Debian netinst ISO
#   vm.sh install   run the installer (opens a window; interactive)
#   vm.sh run       boot headless, console on this terminal
#   vm.sh ssh       ssh into a running VM
#   vm.sh snapshot  snapshot the disk (VM must be shut down)
#
# The VM lives outside the repo — a 60 GiB disk image has no business in a
# source tree. Override with VM_DIR.
set -eu

VM_DIR=${VM_DIR:-$HOME/VMs/sarala-build}
DISK=${DISK:-$VM_DIR/debian.qcow2}
DISK_SIZE=${DISK_SIZE:-60G}
MIRROR=${MIRROR:-https://cdimage.debian.org/debian-cd/current/amd64/iso-cd}

# Host is 6c/12t with 16 GB. Leave macOS real headroom; see build-environment.md.
VCPUS=${VCPUS:-8}
MEMORY=${MEMORY:-8G}
SSH_PORT=${SSH_PORT:-2222}

die() {
	echo "vm.sh: $*" >&2
	exit 1
}

# Arguments common to install and run. HVF is non-negotiable: without it the
# guest falls back to TCG and every build becomes unusable slow. `-cpu host`
# doubles as the check — see `verify_hint`.
qemu_base() {
	echo "-accel hvf -cpu host -smp $VCPUS -m $MEMORY" \
		"-drive file=$DISK,if=virtio,format=qcow2" \
		"-netdev user,id=net0,hostfwd=tcp::$SSH_PORT-:22" \
		"-device virtio-net-pci,netdev=net0"
}

verify_hint() {
	cat <<EOF

Once booted, confirm HVF is actually engaged — the fallback to TCG is silent:

    time python3 -c "sum(range(20000000))"

Under HVF this takes well under a second. Seconds means you are emulating,
and every build will be an order of magnitude slower than it should be.

Timing is the check that cannot be faked. Do not rely on the CPU model name:
HVF does not expose the CPUID brand-string leaves, so lscpu reports a bare
family/model ("06/9e" is Coffee Lake, i.e. correct) rather than a host brand
string, which looks alarming and is not.
EOF
}

# Resolve the current ISO name from SHA256SUMS rather than hardcoding a
# version, so this keeps working across Debian point releases.
iso_name() {
	[ -f "$VM_DIR/SHA256SUMS" ] || die "no SHA256SUMS; run 'vm.sh fetch' first"
	name=$(grep -o 'debian-[0-9.]*-amd64-netinst\.iso' "$VM_DIR/SHA256SUMS" | head -1)
	[ -n "$name" ] || die "no netinst image listed in SHA256SUMS"
	echo "$name"
}

cmd_fetch() {
	mkdir -p "$VM_DIR"
	curl -fsSL -o "$VM_DIR/SHA256SUMS" "$MIRROR/SHA256SUMS"

	iso=$(iso_name)
	if [ -f "$VM_DIR/$iso" ]; then
		echo "already have $iso"
	else
		echo "downloading $iso"
		curl -fL --progress-bar -o "$VM_DIR/$iso" "$MIRROR/$iso"
	fi

	# The ISO bootstraps every byte of toolchain this project depends on.
	# Verifying it is not ceremony.
	(cd "$VM_DIR" && shasum -a 256 -c SHA256SUMS --ignore-missing)
}

cmd_install() {
	iso=$(iso_name)
	[ -f "$VM_DIR/$iso" ] || die "no ISO at $VM_DIR/$iso; run 'vm.sh fetch'"

	if [ -f "$DISK" ]; then
		die "$DISK already exists; delete it to reinstall"
	fi
	qemu-img create -f qcow2 "$DISK" "$DISK_SIZE"

	cat <<'EOF'
In the installer:
  - guided partitioning, whole disk
  - at software selection, DESELECT the desktop environment
  - select only "SSH server" and "standard system utilities"

A build host with a desktop on it is a build host you will regret.
EOF
	verify_hint

	# shellcheck disable=SC2046 # word splitting of qemu_base is intended
	qemu-system-x86_64 $(qemu_base) \
		-cdrom "$VM_DIR/$iso" \
		-boot d \
		-display default,show-cursor=on
}

cmd_run() {
	[ -f "$DISK" ] || die "no disk at $DISK; run 'vm.sh install' first"
	echo "ssh in with: $0 ssh    (monitor: Ctrl-A then C, quit: Ctrl-A then X)"

	# shellcheck disable=SC2046 # word splitting of qemu_base is intended
	exec qemu-system-x86_64 $(qemu_base) \
		-display none \
		-serial mon:stdio
}

cmd_ssh() {
	user=${1:-${VM_USER:-$(id -un)}}
	# The VM is a fresh install reachable only on loopback, and its host key
	# changes on every reinstall. Pinning it buys nothing here.
	exec ssh -p "$SSH_PORT" \
		-o StrictHostKeyChecking=no \
		-o UserKnownHostsFile=/dev/null \
		"$user@localhost"
}

cmd_snapshot() {
	tag=${1:-clean-install}
	[ -f "$DISK" ] || die "no disk at $DISK"
	qemu-img snapshot -c "$tag" "$DISK"
	echo "snapshot '$tag' created; list with: qemu-img snapshot -l $DISK"
}

usage() {
	# The header comment is the help text: print it from below the shebang
	# down to the first line that is not a comment.
	awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
	exit "${1:-0}"
}

case "${1:-}" in
fetch) cmd_fetch ;;
install) cmd_install ;;
run) cmd_run ;;
ssh)
	shift
	cmd_ssh "$@"
	;;
snapshot)
	shift
	cmd_snapshot "$@"
	;;
-h | --help | help | "") usage 0 ;;
*)
	echo "vm.sh: unknown command '$1'" >&2
	usage 1
	;;
esac
