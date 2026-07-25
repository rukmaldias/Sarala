# marlin — boot image geometry

Everything `mkbootimg` needs to package a `boot.img` that marlin's bootloader
(aboot) will accept. Nothing here is flashed: the image is booted transiently
with `fastboot boot`, and Android stays intact on the other slot.

## Source

LineageOS `android_device_google_marlin`, `lineage-22.2`,
`BoardConfigCommon.mk`.

## Geometry

| Parameter | Value | Notes |
|---|---|---|
| base | `0x80000000` | DRAM base |
| kernel_offset | `0x00080000` | `BOARD_KERNEL_OFFSET := 0x80000` |
| ramdisk_offset | `0x02700000` | |
| tags_offset | `0x02500000` | where the dtb/atags land |
| pagesize | `4096` | |
| header version | **0** | no `BOARD_BOOT_HEADER_VERSION` set → v0 |

## The dtb is appended to the kernel

`BOARD_KERNEL_IMAGE_NAME := Image.lz4-dtb` — the kernel image is the kernel with
the **dtb concatenated onto the end**. Boot-image header v0 has no separate dtb
field (that arrived in v2), so on marlin the dtb *must* ride with the kernel.
aboot scans for the dtb magic after the kernel and matches by board-id.

**The kernel MUST be compressed (gzip or lz4), not raw.** aboot only runs its
appended-dtb search *after decompressing the kernel* — with a raw uncompressed
`Image` it never looks for the dtb and fails with `dtb not found` regardless of
correct IDs. This was the multi-hour trap on first boot (2026-07-24): raw,
correct-ID, correctly-padded images all failed; the identical image with a
**gzip**-compressed kernel booted immediately. gzip is self-delimiting, so aboot
finds the appended dtb cleanly after the gzip stream. (Earlier advice to "start
uncompressed, simplest" was wrong — deleted.)

```sh
gzip -n -9 -c Image > Image.gz
cat Image.gz msm8996pro-google-marlin.dtb > Image.gz-dtb
```

Note also: a raw arm64 `Image`'s header declares `image_size` (offset 0x10)
**larger than the file** (it includes BSS). If ever appending to a raw Image, it
must be zero-padded to `image_size` first or the dtb lands at the wrong offset.
The gzip path sidesteps this entirely.

## Packaging recipe

```sh
mkbootimg \
  --kernel        Image-dtb \
  --ramdisk       initramfs.cpio.gz \
  --base          0x80000000 \
  --kernel_offset 0x00080000 \
  --ramdisk_offset 0x02700000 \
  --tags_offset   0x02500000 \
  --pagesize      4096 \
  --header_version 0 \
  --cmdline "earlycon console=ttyMSM0,115200n8" \
  -o boot.img

fastboot boot boot.img      # transient — flashes nothing
```

(With `--kernel Image.gz-dtb` per the note above — not a raw `Image-dtb`.)

## First on-hardware boot — confirmed (2026-07-24)

The first `fastboot boot` on the physical Pixel XL. aboot accepted the image,
**matched our dtb, and jumped to our kernel.** Captured over the serial console
([`serial-console.md`](serial-console.md)). What the aboot log established:

- **dtb match (authoritative IDs).** aboot logged
  `Best match DTB tags 422/00000080/0x00000000/10001/20009/455013/0/0`
  → msm-id **422**, board-id variant **0x80** / subtype **0**, soc_rev
  **0x10001** (MSM8996 Pro **v1.1**), pmic `0x20009/0x455013`. These are now in
  the dts. The initial `0x10000` (v1.0) guess did *not* match — the rev must be
  exact; best-fit `<=` did not save it.
- **aboot reached** `booting linux @ 0x80080000` → `Jumping to kernel via
  monitor`. Geometry (base/offsets/pagesize/header-v0) all accepted as recorded
  above.

### Two hurdles now on the front line

Full narrative and everything ruled out: [`first-boot.md`](first-boot.md).

1. **No kernel output — the kernel faults before `earlycon`.** Silent after the
   jump. Ruled out: cmdline (bare `earlycon` is the correct form, not
   `earlycon=msm_serial_dm,…`), kernel config (earlycon *is* built in), DTB
   nodes (`/cpus`/PSCI/timer/GIC/memory all present), and VA_BITS/KASLR (a
   rebuild with `VA_BITS_48` + KASLR off — the ARMv8.0-appropriate config — is
   still silent). Since arm64 has no pre-earlycon debug, this is the wall. Next:
   diff against a known-good msm8996-mainline device.

2. **aboot forces `skip_initramfs` (system-as-root / A-B).** The log shows aboot
   appending `rootwait skip_initramfs init=/init` and a dm-verity
   `root=/dev/dm-0 ... android-verity /dev/sda34` for `system_b`. Left as-is the
   kernel will mount the real Android system and **ignore our initramfs**, so it
   never reaches Sarala's `/init`. To get the stage-1 shell we must boot in a
   mode aboot treats as recovery (where it does *not* add `skip_initramfs`).
   Deferred until earlycon works, but recorded so it isn't a surprise.

## Command line

Stock marlin cmdline, for reference:

```
console=ttyHSL0,115200,n8 androidboot.console=ttyHSL0 ehci-hcd.park=3 \
lpm_levels.sleep_disabled=1 cma=32M@0-0xffffffff loop.max_part=7 \
androidboot.boot_devices=soc/624000.ufshc
```

Two facts it confirms:

- **Debug console is blsp1_uart2.** `ttyHSL0` is the downstream driver name for
  the UART at **0x7570000** — which is `blsp1_uart2`, exactly what the device-
  tree skeleton encodes. In mainline the same UART enumerates as `ttyMSM0`, so
  the mainline cmdline uses `console=ttyMSM0` + `earlycon=msm_serial_dm,0x7570000`.
- **UFS boot device is `soc/624000.ufshc`** — matches the `ufshc@624000` node
  found in the SoC dtsi (see [`hardware.md`](hardware.md)).

The Android-specific arguments (`androidboot.*`, `loop.max_part`) are dropped
for Sarala; the initramfs supplies `/init` and boots from the ramdisk, not UFS,
so no boot-device argument is needed for the first signal.
