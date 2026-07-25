# marlin — first on-hardware boot (bring-up log)

Chronological record of the first attempts to `fastboot boot` a Sarala image on
the physical Pixel XL and read the console. Nothing is flashed; every boot is
transient and Android stays intact. Companion to [`boot-image.md`](boot-image.md)
(packaging) and [`serial-console.md`](serial-console.md) (how the console is
read).

**Status:** aboot accepts our image, matches our dtb, and jumps to our kernel.
The kernel now **executes** (it no longer silently hangs) but dies early with a
**`NOC_ERROR` / `RPM:TZ ABORT!`** — a TrustZone XPU/interconnect fault — before
`earlycon` produces output. Front line: the memory map / kernel config (see
"Reference diff" below).

## Method

- Serial console on the 3.5 mm jack, `oem uart enable`, read directly from the
  Mac at 115200 (`/dev/cu.usbserial-3`) with a small raw-`termios` capture
  script rather than an interactive terminal, so the full boot is logged to a
  file. **Use `cu.*`, not `tty.*`** (see serial-console.md).
- Capture *the whole window*: start the capture with the phone already in
  fastboot, then `fastboot boot`. Watch that the post-jump period is actually in
  the capture — early runs had the jump land at the very end of the window and
  captured nothing after it.

## What works — the bootloader accepts us and jumps

- **dtb match (authoritative IDs, from aboot's own log):**
  `Best match DTB tags 422/00000080/0x00000000/10001/20009/455013/0/0`
  → msm-id **422**, board-id variant **0x80** / subtype **0**, soc_rev
  **0x10001** (MSM8996 Pro **v1.1**), pmic `0x20009/0x455013`. Now in the dts.
- **aboot runs to** `booting linux @ 0x80080000` → `Jumping to kernel via
  monitor`. Geometry (base/offsets/pagesize/header-v0) all accepted.

## Hard-won packaging rules (each cost real time)

1. **The kernel MUST be compressed (gzip/lz4), not raw.** aboot only searches
   for the appended dtb *after decompressing the kernel*; a raw `Image` is never
   scanned → `dtb not found` regardless of correct IDs. Tested exhaustively:
   raw and lz4 both failed to be found/booted; **gzip booted**. gzip is
   self-delimiting, so aboot locates the appended dtb cleanly after the stream.
2. **soc_rev must match exactly.** `0x10000` (v1.0) did not match this v1.1
   unit; best-fit `<=` did not save it. The device is MSM8996 Pro **v1.1**
   (`/sys/devices/soc0/revision` = "1.1", soc_id 305 via socinfo, but aboot
   matches msm-id **422/423** — the socinfo id and the msm-id differ).
3. **image_size padding (raw only).** A raw arm64 `Image` header declares
   `image_size` > file size (BSS); appending a dtb by `cat` puts it at the wrong
   offset. Irrelevant on the gzip path, which is what we use.
4. **earlycon is bare `earlycon`, not `earlycon=msm_serial_dm,0x…`.** Mainline
   `msm_serial.c` registers only `OF_EARLYCON_DECLARE(msm_serial_dm,
   "qcom,msm-uartdm", …)` — matched via the DT `stdout-path`, not by the named
   `earlycon=` form (no bare `EARLYCON_DECLARE`). Our dts sets
   `stdout-path = "serial0:115200n8"` → `serial@7570000`
   (`compatible = "qcom,msm-uartdm"`), which is what bare `earlycon` needs.

## The wall — silent before earlycon

After `Jumping to kernel via monitor`, the jack is **silent** (a newline, then
nothing — not even floating-line noise). Ruled out, in order:

- **Not the cmdline.** Bare `earlycon` (correct form) is silent too.
- **Not kernel config.** The embedded `.config` (extracted from the `Image` via
  IKCONFIG) has `CONFIG_SERIAL_MSM=y`, `SERIAL_MSM_CONSOLE=y`,
  `SERIAL_EARLYCON=y`, `OF_EARLY_FLATTREE=y`. earlycon *is* built in.
- **Not a missing DTB node.** The flattened dtb has `/cpus` with
  `enable-method = "psci"`, PSCI, `arm,armv8-timer`, GIC, `/memory`, `/chosen`.
- **Not (obviously) VA/KASLR.** The stage-0 build used the stock arm64
  `defconfig` (tuned for the modern ARMv8.2+ CPU that QEMU `virt` emulates), but
  marlin is **ARMv8.0 Kryo**. Rebuilt with `CONFIG_ARM64_VA_BITS_48`,
  `PA_BITS_48`, and `RANDOMIZE_BASE` off (both run in `head.S`, before any
  console) — **still silent.** So VA_BITS_52/KASLR was not the (whole) cause.

Because the config, DTB, and cmdline are all correct, and bare earlycon runs
before any pinctrl/clock driver could re-route the jack, the most consistent
explanation is that **the kernel faults in early `head.S`, before earlycon
exists.** On arm64 there is no pre-earlycon debug (no `DEBUG_LL`), so this is
invisible without JTAG or a known-good reference to diff against.

Lesson: **the kernel `.config` is part of the port.** `defconfig` is a stage-0
(QEMU) convenience and does not carry to real MSM8996.

## Second, compounding blocker — `skip_initramfs`

aboot force-appends (seen in its log):
`rootwait skip_initramfs init=/init` and
`root=/dev/dm-0 dm="… android-verity /dev/sda34"` for `system_b`.
This is marlin's system-as-root / A-B behaviour: a *normal* boot skips the
ramdisk and mounts the real dm-verity Android system. So even a fully-booting
kernel would **bypass our initramfs** and panic mounting a system it can't read.
To reach Sarala's `/init` we must boot in a mode aboot treats as **recovery**
(where it does not add `skip_initramfs`).

## Reference diff vs oneplus3 (msm8996-mainline, boots via stock aboot)

Compared our build to postmarketOS `device-oneplus-oneplus3` +
`linux-postmarketos-qcom-msm8996`. Findings:

- **Kernel config (their config *boots* mainline on msm8996):**
  - `CONFIG_ARM64_VA_BITS=48` — confirms our VA48 fix was right (defconfig's 52
    is wrong for ARMv8.0).
  - `CONFIG_RANDOMIZE_BASE=y` — KASLR is **kept on**, so it was a red herring;
    disabling it was harmless but unnecessary.
  - **`# CONFIG_EFI is not set`** — pmOS disables EFI; our defconfig has it on.
    A real delta to try.
- **boot.img offsets differ** (theirs, which boots):
  `kernel 0x00008000, ramdisk 0x01000000, tags 0x00000100` vs our marlin/
  LineageOS `kernel 0x00080000, ramdisk 0x02700000, tags 0x02500000`. Ours put
  the dtb/ramdisk **inside the 41 MB kernel's load span** — the kernel image was
  being clobbered, hence the earlier silent hang.

### What the diff produced

Moving the dtb/ramdisk clear of the kernel span (dtb low @ `0x80000100`,
ramdisk @ `0x83000000`) changed the failure from **silent hang** to **the kernel
running and taking a `NOC_ERROR` / `RPM:TZ ABORT!`** (SNOC/PNOC interconnect
error, secure warm-reset, TZBSP crash log dumped by the restarted LK). A **PNOC
(peripheral) NOC error before earlycon** points at an early access to a
peripheral that is unclocked/XPU-protected — plausibly the earlycon UART write
itself, or the kernel's early setup touching a region that marlin's TrustZone
protects but our DTB doesn't reserve.

Two compounding root issues:
- **Fat kernel.** 41 MB (defconfig) leaves no room for boot artifacts in
  marlin's known-usable low DRAM (`base … base+~40 MB`, per the downstream
  offsets); the kernel nearly fills it. A lean config shrinks this.
- **Generic reserved-memory.** Our DTB carries the *generic* mainline msm8996
  carveouts (smem@86000000, mpss@88800000, mba@91500000, …). marlin/HTC's actual
  TZ-protected map likely differs; regions TZ protects but we don't reserve →
  XPU/NOC abort.

## Next directions

1. **Rebuild with a lean, msm8996-tuned config** (pmOS's, adapted to 6.16, with
   `CONFIG_EFI` off): smaller kernel that fits under the known-good downstream
   offsets, and a config proven to boot msm8996. Then repack with the original
   marlin offsets (`kernel 0x80000, ramdisk 0x2700000, tags 0x2500000`).
2. **Correct the memory map:** derive marlin's real reserved-memory carveouts
   from the downstream device tree and match `/memory` + `reserved-memory`.
3. **Decode the NOC ERRLOG** (`SNOC ERRLOG0=0x80030108`, `PNOC ERRLOG1=0x0ac01005`)
   to identify the exact faulting master/peripheral.
4. **Defeat `skip_initramfs`** (recovery-mode boot) so we reach `/init` once the
   console lives.
