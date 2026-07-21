# marlin device tree

Sarala owns marlin's device tree — it is written code, kept here rather than
carried as a patch against the kernel. See [`../hardware.md`](../hardware.md)
for the hardware facts each node is built from.

## Files

| File | Contents |
|---|---|
| `msm8996pro-google-marlin.dts` | Stage 1 skeleton: boot + serial console only |

Later this splits into a board `.dtsi` plus per-panel `.dts` variants, mirroring
the OnePlus 3T template (`s6e3fa3` / `s6e3fa5`) — marlin's panel lottery needs
the same pattern. Not yet: the skeleton has no panel.

## Building it

The device tree compiles inside the kernel tree (it `#include`s
`msm8996pro.dtsi`). Until a build script wraps this, the manual steps are:

```sh
# in the kernel tree (~/src/linux)
cp .../boards/marlin/dts/msm8996pro-google-marlin.dts \
   arch/arm64/boot/dts/qcom/
# register it in arch/arm64/boot/dts/qcom/Makefile:
#   dtb-$(CONFIG_ARCH_QCOM) += msm8996pro-google-marlin.dtb
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- \
     qcom/msm8996pro-google-marlin.dtb
```

Verified to compile against Linux 6.16.0-rc2 (`msm8996-staging`), producing a
58 KB dtb. The Stage 0 kernel config already carries `CONFIG_SERIAL_MSM`,
`CONFIG_SERIAL_MSM_CONSOLE`, and `CONFIG_SERIAL_EARLYCON`, so no config change
is needed to drive the console.

## First-signal strategy: earlycon

The skeleton omits the entire regulator tree on purpose. `earlycon` prints from
the UART the bootloader already initialised, before any driver or regulator
probes — the earliest possible signal, and enough to prove the dtb boots. The
regulator tree arrives with the peripherals that actually need it (UFS, USB,
panel).

## Open VERIFY items

Marked inline in the `.dts`; the load-bearing ones:

- **Which UART is the debug console** — blsp1_uart2 (0x07570000, the encoded
  guess, from the downstream Qualcomm base) vs blsp2_uart2 (0x075b0000, which
  the OnePlus 3T uses). And how it is physically reached on a Pixel: internal
  test points or the USB-C sideband, not a header pin.
- **The full memory map** — a conservative low range is encoded; aboot patches
  the node at boot, but confirm before relying on all 4 GB.
- **board-id / msm-id** — only needed once we build a multi-dtb boot image;
  irrelevant to a single dtb passed via `fastboot boot`.
