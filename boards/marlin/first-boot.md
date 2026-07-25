# marlin — first on-hardware boot (bring-up log)

Chronological record of the first attempts to `fastboot boot` a Sarala image on
the physical Pixel XL and read the console. Nothing is flashed; every boot is
transient and Android stays intact. Companion to [`boot-image.md`](boot-image.md)
(packaging) and [`serial-console.md`](serial-console.md) (how the console is
read).

**Status:** SOLVED what blocked early boot — **our skeleton DTB was the
problem.** Booting a *complete* mainline dtb (oneplus3's, with marlin's IDs
patched in) runs the kernel on marlin all the way to a **working console on
`ttyMSM0` @ `0x7570000` (the jack)** and ~2.7 s of driver init. The skeleton's
flat 2 GB `/memory` + minimal reserved-memory faulted the kernel very early
(the `NOC_ERROR`/`TZ ABORT`). Remaining crash with the oneplus3 dtb: probing
the *second* UART `75b0000.serial` (blsp2) NOC-aborts. Front line: build a
proper marlin dts (real memory map + reserved-memory, `console=ttyMSM0`, and
don't probe blsp2).

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

### Rebuilt with pmOS's msm8996 config — NOC_ERROR persists

Built the kernel from postmarketOS's `config-postmarketos-qcom-msm8996` (adapted
to 6.16, `EFI` off, `VA_BITS_48`): a lean **25 MB** kernel (ends `~0x81a50000`),
repacked with the **original downstream offsets** (tags `0x82500000`, ramdisk
`0x82700000` — now clear of the kernel, no collision). **Same `NOC_ERROR` /
`RPM:TZ ABORT!` before earlycon.**

So the fault is **not** config, kernel size, or artifact placement — all now
match a known-good msm8996 setup. The kernel executes and takes an early
**PNOC (peripheral) interconnect fault** that TrustZone rejects, before any
console. This is a genuine, marlin-specific early hardware access — the stock
*and* downstream kernels reach this UART (we saw stock Android dmesg on the jack),
so it is something the *mainline* early path touches that marlin's TZ/XPU blocks.

Note: on the abort, LK writes a **RAMDUMP** to the `ramdump` partition
(`RAMDUMP_MSG.txt` + CPU register context) — a potential source for the exact
faulting PC/address.

### Isolation tests that cracked it (#2, #3)

- **#2 — earlycon is not the fault.** Booting with `earlycon` removed still
  `NOC_ERROR`s, so the console path is not the culprit.
- **#3 — a complete dtb boots; the skeleton does not.** Compiled oneplus3's
  `msm8996-oneplus3.dtb`, patched its `qcom,msm-id`/`board-id` to marlin's,
  paired it with our lean pmOS kernel, and booted it on marlin. Result:

  ```
  [0.000000] Booting Linux on physical CPU 0x0 [0x512f2011]   (Kryo)
  [0.000000] Machine model: OnePlus 3
  [0.000000] earlycon: msm_serial_dm0 at MMIO 0x075b0000
  ...
  [2.720317] printk: legacy console [ttyMSM0] enabled          (0x7570000 = jack!)
  ...
  [2.731952] msm_serial 75b0000.serial: detected port #1
  [2.735136] msm_serial 75b0000.serial: uartclk = <abort → LK restart>
  ```

  So: the kernel + our pmOS config are fine; marlin's TZ/hardware is fine to a
  working console. **The skeleton DTB was the blocker.** oneplus3's dtb carries
  the real memory map and extra reserved regions (rmtfs, mpss-metadata,
  ramoops@ac000000 — note the earlier NOC addr `0x0ac0…`); ours reserved none of
  those and claimed a flat 2 GB, so the kernel touched protected memory early.
  The one remaining crash is the probe of the **second UART `75b0000.serial`
  (blsp2)** — its clock/registers NOC-abort on marlin (not routed/clocked). The
  console we *want* — `ttyMSM0` @ `0x7570000` — already works over the jack.

## Next directions (updated)

1. **Build a proper marlin dts** — the clear path now: real `/memory` +
   `reserved-memory` (derive marlin's actual carveouts; the generic skeleton set
   is insufficient), `console=ttyMSM0` (0x7570000, proven over the jack), and
   ensure blsp2 (`75b0000.serial`) is not probed (disabled / not clocked). Crib
   structure from oneplus3, keep marlin's IDs.
2. **Defeat `skip_initramfs`** (recovery-mode boot) — still needed downstream of
   the console, so the kernel runs Sarala's `/init` instead of mounting stock
   Android's dm-verity system.

(Done and folded in above: #2 earlycon-isolation, #3 downstream/complete-dtb
comparison. Decoding the NOC ERRLOG / reading the ramdump is now optional — the
oneplus3-dtb boot already localised the cause to the skeleton's memory map.)
