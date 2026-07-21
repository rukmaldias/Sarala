# Stage 0 — build notes

What actually worked, recorded so a clean rebuild does not rediscover the
sharp edges. This is the log of a completed stage, not a design document; the
design lives in [`build-environment.md`](build-environment.md) and
[`roadmap.md`](roadmap.md).

**Exit criterion met:** a PID 1 written in Rust boots the mainline kernel
under `qemu-system-aarch64 -M virt` and hands back an interactive shell.
`cat /proc/1/comm` reports `init` — our binary, genuinely PID 1.

## Versions this was done against

| Component | Version |
|---|---|
| Kernel | 6.16.0-rc2, `msm8996-staging` @ `4adb68401` |
| Cross gcc | `aarch64-linux-gnu-gcc` 14.2.0 (Debian) |
| Rust | 1.97.1, target `aarch64-unknown-linux-musl` |
| busybox | 1.37.0, static |
| QEMU | 10.0.2 (build host and target) |

## Where things run

- **Build** — inside the x86_64 Debian VM. Kernel tree at `~/src/linux`,
  busybox at `~/src/busybox-1.37.0`, the Sarala source at `~/Sarala`.
- **Boot** — on the macOS host, not nested in the VM. Only two finished
  artifacts cross the boundary: `Image` and `initramfs.cpio.gz`.

## Kernel

```sh
cd ~/src/linux
export ARCH=arm64 CROSS_COMPILE="ccache aarch64-linux-gnu-"
make defconfig          # arm64 defconfig already supports -M virt
make -j8 Image          # first build ~19 min; ccache makes rebuilds cheap
# -> arch/arm64/boot/Image
```

The stock arm64 `defconfig` already carries everything stage 0 needs. Verified
present before the long build, since each of these fails *silently* rather than
erroring: `CONFIG_DEVTMPFS(_MOUNT)`, `CONFIG_VIRTIO_MMIO/_BLK/_NET`,
`CONFIG_SERIAL_AMBA_PL011(_CONSOLE)`, `CONFIG_BLK_DEV_INITRD`, `CONFIG_TMPFS`,
`CONFIG_BINFMT_ELF`. `CONFIG_DEVPTS_FS` has no symbol in modern kernels — devpts
is built in unconditionally with `CONFIG_UNIX98_PTYS`; its absence from
`.config` is expected, not a problem.

## busybox — two cross-compile gotchas

Neither is documented upstream; both cost time the first time.

1. **Disable the x86 SHA hardware-accel paths.** With `defconfig`,
   `CONFIG_SHA1_HWACCEL` / `CONFIG_SHA256_HWACCEL` pull in x86-only SHA-NI code
   (`sha1_process_block64_shaNI`), which is *undeclared* on aarch64 and breaks
   the build in `libbb/hash_md5_sha.c`. Turn both off.
2. **Disable `CONFIG_TC`.** The `tc(8)` applet does not compile against modern
   kernel headers. We do not need it.

```sh
cd ~/src/busybox-1.37.0
make defconfig
sed -i 's/# CONFIG_STATIC is not set/CONFIG_STATIC=y/'            .config
sed -i 's/^CONFIG_SHA1_HWACCEL=y/# CONFIG_SHA1_HWACCEL is not set/'   .config
sed -i 's/^CONFIG_SHA256_HWACCEL=y/# CONFIG_SHA256_HWACCEL is not set/' .config
sed -i 's/^CONFIG_TC=y/# CONFIG_TC is not set/'                   .config
make oldconfig
make -j8 CROSS_COMPILE=aarch64-linux-gnu-
# -> busybox, static aarch64 ELF, ~2.1M
```

Static on purpose: the initramfs then needs no libc alongside it. Busybox is
scaffolding — it is removed entirely in stage 3 (see roadmap).

## PID 1 and the cross-linker

```sh
cd ~/Sarala
cargo build --release    # target pinned by .cargo/config.toml
# -> target/aarch64-unknown-linux-musl/release/init, static, ~389K
```

The crate *type-checks* on any host but only *links* where an aarch64 linker
exists. `.cargo/config.toml` names `linker = "aarch64-linux-gnu-gcc"`; without
it cargo uses the host `cc`, whose x86_64 `ld` rejects the aarch64 objects with
`Relocations in generic ELF (EM: 183)`. This is why the first real link happens
in the VM, never on macOS.

## Initramfs and boot

```sh
# in the VM
cd ~/Sarala
BUSYBOX=~/src/busybox-1.37.0/busybox ./scripts/mkinitramfs.sh
# -> out/initramfs.cpio.gz

# copy Image + initramfs.cpio.gz to the macOS host's out/, then on the host:
./scripts/run-qemu.sh
```

`mkinitramfs.sh` links only a handful of applets (`sh ls cat mount umount
dmesg ps mkdir echo sleep`). `uname` and `head` are deliberately absent so far —
widen the applet list in the script when something needs them.

Boot signature that means success:

```
Run /init as init process
[init] sarala-init 0.0.1
[init] /bin/sh started as pid 58
/ #
```
