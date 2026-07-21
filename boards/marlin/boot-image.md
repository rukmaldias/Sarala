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

Stock compresses with lz4; aboot also accepts a raw `Image` and gzip. Start
uncompressed — simplest, one fewer variable when chasing first boot:

```sh
cat Image msm8996pro-google-marlin.dtb > Image-dtb
```

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
  --cmdline "earlycon=msm_serial_dm,0x7570000 console=ttyMSM0,115200n8" \
  -o boot.img

fastboot boot boot.img      # transient — flashes nothing
```

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
