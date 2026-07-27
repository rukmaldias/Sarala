# marlin — first on-hardware boot (bring-up log)

Chronological record of the first attempts to `fastboot boot` a Sarala image on
the physical Pixel XL and read the console. Nothing is flashed; every boot is
transient and Android stays intact. Companion to [`boot-image.md`](boot-image.md)
(packaging) and [`serial-console.md`](serial-console.md) (how the console is
read).

**Status:** **Kernel boots with a live serial console, deep into
`deferred_probe`.** Disabled so far: MMSS/LPASS IOMMUs, the dead BLSP2 block,
the GPU stack, and the **SMP2P IPC to the DSPs** (a real async-fault source).
Also needs `clk_ignore_unused` on the cmdline (else `clk_disable_unused` WARNs
on `fd_ahb_clk`, a CAMSS clock under a powered-off domain). **Remaining
blocker:** a residual **async `NOC_ERROR`/TZ-abort** reset around the
i2c/cpufreq stage — the crash point moves with logging verbosity, so it's an
IRQ/callback from a still-running remote or an unpowered block, not yet pinned.
The power framework itself (rpmpd/genpd/regulators/icc) is up, so it's not a
blanket power-domain failure.

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

## Building the marlin dts — the skeleton is too minimal (2026-07-25)

Tried to fix the skeleton incrementally, rebuilding each dtb from source in the
kernel tree and booting on hardware. Ruled out, one variable at a time — **each
still `NOC_ERROR`/`TZ ABORT`s before any console output:**

- `/memory` size 0 (defer to aboot, matching oneplus3) — no change.
- reserved-memory matched to oneplus3, incl. `ramoops@ac000000` — no change.
- SoC base swapped `msm8996pro.dtsi` → `msm8996.dtsi` — no change.

Meanwhile the **complete** oneplus3 dtb (marlin IDs) boots to 2.7 s and a
working `ttyMSM0` console. The remaining difference is that oneplus3 **enables a
full node set** (regulators, RPM/SMD, PMIC, USB, …) whereas our skeleton enables
essentially only `blsp1_uart2`. So the kernel crashes at an early driver step
that a complete dts gets past — the skeleton is **too minimal**, not
mis-configured in memory/base/config.

**Revised approach:** build marlin's dts *up from a complete sibling* — the
roadmap's recommended `msm8996pro-oneplus3t.dtsi` (same MSM8996 **Pro** silicon)
— keeping marlin's IDs, `console=ttyMSM0` on `blsp1_uart2` (the jack), and
leaving `blsp2 (75b0000)` disabled. Trim board-specifics (panel/touch/battery)
later; they probe long after the serial console we need for stage 1.

## oneplus3t-based dts — kernel log over serial (2026-07-26)

Built marlin's dts up from the Pro sibling substrate and boot-tested each step
on hardware. What the isolation showed:

- **`msm8996pro.dtsi` + `msm8996-oneplus-common.dtsi` + marlin IDs boots** — to
  `ttyMSM0` on the jack at ~2.7 s, 400+ kernel log lines. This is the current
  `dts/msm8996pro-google-marlin.dts`.
- **The Pro base is fine** — pro-vs-nonpro was not the issue; the substrate
  (regulators/PMIC/pinctrl from oneplus-common) is what the skeleton lacked.
- **Adding just the PM8994 RPM regulators to the skeleton was NOT enough** — the
  full common substrate is needed (something beyond the regulators, in the
  node-enable set, matters).
## Resolving the blsp2 crash + the console knot (2026-07-26)

First, the "capture flakiness": `oem uart enable` does **not** flood the UART
(8 s idle after it = ~380 bytes). The big captures were the *boot's* crash/reset
noise, and the misleading "jump at 100% of bytes" is just because a crash dumps
a lot then goes quiet. Clean method: start the capture, then `oem uart enable`,
then `fastboot boot`; analyse by content, not byte-offset.

Then the console knot, isolated on hardware:

- **`earlycon` is load-bearing** — with it removed (even on the working Test A
  dtb) the kernel goes silent and hangs before the `ttyMSM0` driver comes up at
  ~2.7 s. (So the earlier "no earlycon, rely on ttyMSM0" idea was wrong.)
- **earlycon is OF-only** — `msm_serial.c` has just `OF_EARLYCON_DECLARE`, no
  address-based `EARLYCON_DECLARE`. So `earlycon=msm_serial_dm,ADDR` does
  nothing; earlycon must bind via DT `stdout-path` to a `qcom,msm-uartdm` node.
- **earlycon on blsp1 (the jack, 0x7570000) faults early**; earlycon on blsp2
  (0x75b0000) works. (Opposite of intuition — likely the jack UART's LK/oem-uart
  state vs blsp2's default state; not fully explained.)
- **blsp2's driver probe NOC-aborts** (its registers aren't accessible on
  marlin) — so it must be disabled.

**The fix (works):** disable blsp2's *driver* (`status = "disabled"`) but keep
`stdout-path = serial1` (blsp2) — `of_setup_earlycon` binds earlycon to the
node's `reg` regardless of status, so earlycon still lives on 0x75b0000 (load-
bearing) while the platform driver skips blsp2 (no probe crash). Boot cmdline:
`earlycon console=ttyMSM0,115200n8`. Visible console is `ttyMSM0` on the jack
from ~2.7 s. **Result: boots past blsp2 to ~2.79 s.**

## Iterating past NOC aborts (2026-07-26)

The pattern: peripherals whose power domain/clock the bootloader left off
NOC-abort when the driver touches their registers. For a stage-1 serial boot we
just disable the ones we don't need; each fix reveals the next.

- **IOMMUs.** `arm-smmu@d00000` (`mdp_smmu`) faulted; so do `venus_smmu`
  (d40000) and `vfe_smmu` (da0000) — the MMSS multimedia SMMUs — and
  `lpass_q6_smmu` (1600000, audio). `adreno_smmu` (b40000, GPU) probes fine.
  Disabling the four MMSS/LPASS SMMUs → **verified past all SMMUs to ~2.94 s.**
- **BLSP2 is dead on marlin.** Next fault was `75b5000.i2c` — a BLSP2 I2C bus.
  With blsp2_uart2, the whole BLSP2 block (`0x75bxxxx`) is inaccessible (register
  access NOC-aborts); BLSP1 I2C (7577000/757a000) is fine. Disabled the enabled
  BLSP2 I2C buses (`blsp2_i2c1`, `blsp2_i2c6`) — **VERIFIED: boot now clears all
  NOC aborts, no reset, reaches ~2.97 s.**
- **`initcall_debug` is the key tool here.** The apparent "2.97 s hang" was
  *not* a hang — with `initcall_debug` on the cmdline the kernel is seen running
  deep into `deferred_probe` (~13 s), doing slow *silent* probes (clock-
  controller 109 ms, serial 48 ms, PHYs deferring `-517`) with no console output
  in between. The last completed `probe of X returned` before the reset points
  at the crasher; the next probe NOC-aborts before it can print.
- **GPU stack.** At ~13 s `adreno_smmu` (b40000) is re-probed during
  `deferred_probe` (GPU consumer pulls it in) and NOC-aborts — even though it
  probed fine on the first pass. Disable `&gpu` + `&adreno_smmu` (graphics
  unused for stage-1).
- **Next: an I2C *device* on BLSP1.** After that, a silent NOC-abort reset right
  after `bq27541`/`1-0055` (fuel gauge, ~0x55) on a BLSP1 bus. An aggressive
  trim (disabling display/camera/video/modem/pcie/audio/usb/ufs too) did **not**
  get past it — so the crasher is on the kept BLSP1 i2c path, not the heavy
  peripherals.
- **Hypothesis worth testing:** the sheer number of peripherals that NOC-abort
  suggests a *systemic* power issue — GDSC/rpmpd power-domains the bootloader
  left off and the mainline power-domain drivers aren't bringing up on marlin —
  rather than N independent faults. Fixing that could clear many at once; vs.
  the current disable-each-in-turn approach.
- **The crash is timing-sensitive — evidence it's systemic/async.** To name the
  post-i2c crasher, added `dyndbg`-style per-probe logging. The extra UART
  output (at 115200) slowed the boot and the crash **moved earlier** — reset at
  `smp2p-mpss` (~15.9 s) instead of the i2c area (~13 s, consistent across the
  non-verbose runs). A crash point that shifts with logging is an **async fault**
  (an IRQ / remoteproc / SMP2P callback from an unpowered subsystem), not a
  deterministic per-device probe. Whack-a-mole (disable the next device) will
  not converge on a moving target. **Recommend pivoting to the power-domain /
  interrupt-source investigation** rather than disabling more nodes.

### Ops note — recovering a wedged phone

Repeated crash-boots can leave the phone off the USB bus (no adb/fastboot, not
enumerating) and not auto-recovering. Force a reboot: **hold Power + Volume-Down
~10–15 s.** It returns to Android (or bootloader). Also: after a crash-boot the
phone takes ~30 s+ to reset to Android, so wait for adb/fastboot before the next
`fastboot boot` (racing it fails with "no devices").

## SMP2P async fault + clk_disable_unused + residual async NOC (2026-07-27)

Investigation results (power framework is up — `rpmpd`/`genpd`/`rpm_smd`
regulators/`icc_smd_rpm` all probe fine — so not a blanket power-domain fail):

- **Named the async source via `dyndbg`.** `dyndbg="file drivers/base/dd.c +p"`
  logs each probe *before* it runs. The last one before the reset was
  **`smp2p-slpi`** (and the crash point moved earlier as logging increased —
  confirming async). SMP2P is the shared-memory IPC to the DSPs (ADSP/MPSS/SLPI),
  which are **still running from aboot** and send SMP2P interrupts; our handler
  NOC-aborts. Disabled the three `smp2p-*` channels (by path — no labels).
- **`clk_disable_unused` WARN.** With SMP2P off, the next issue surfaced:
  `clk_disable_unused` → `fd_ahb_clk status stuck at 'on'` (WARN, clk-branch.c),
  a CAMSS clock whose power domain is off. Worked around with `clk_ignore_unused`
  on the cmdline.
- **Residual async NOC fault remains.** Even with SMP2P off + `clk_ignore_unused`,
  a silent `NOC_ERROR`/`RPM:TZ ABORT!` reset still hits around the i2c/cpufreq
  stage, and its point still shifts with logging — a *second* async source
  (another remote IRQ / glink / cpufreq-apcs / interconnect QoS). TZ resets
  instantly on the NOC error, so there's no kernel backtrace to read.

### Ops note — recovering a wedged phone

Repeated crash-boots can leave the phone off the USB bus (no adb/fastboot, not
enumerating) and not auto-recovering. Force a reboot: **hold Power + Volume-Down
~10–15 s.** It returns to Android (or bootloader). Also: after a crash-boot the
phone takes ~30 s+ to reset to Android, so wait for adb/fastboot before the next
`fastboot boot` (racing it fails with "no devices").

## Kernel reaches `/init` — trim the board peripherals + kill DVFS (2026-07-27)

The residual async NOC turned out to be **two ordinary peripheral faults** the
console just hadn't reached yet. Trimming oneplus board nodes we don't use
(step #2) both shrank the masquerade *and* peeled them off one at a time — and
this session the bootloader finally handed us the missing backtrace: HTC aboot
dumps the **NOC error registers** after the reset (`SNOC/PNOC ERROR: ERRLOGn`),
so the faults are no longer silent.

- **Fault 1 — oneplus i2c buses (PNOC abort).** With the big peripherals off
  (display/camera/audio/PCIe/USB/UFS/`mss_pil`), the last kernel line before the
  reset was `i2c_qup 7577000.i2c` / `757a000.i2c`, then `PNOC ERROR ERRLOG1 =
  0x0a801204` (a BLSP QUP route). Those are `blsp1_i2c3` (tfa9890 amp) and
  `blsp1_i2c6` (bq27541 fuel gauge) — **oneplus-only devices marlin lacks**;
  bringing up the QUP to reach them NOC-aborts. Disabled both buses.
- **Fault 2 — CPU DVFS (cpufreq).** Next reset landed exactly at
  `cpufreq_policy_online: CPU0 … changing to 1056000 kHz`. `qcom-cpufreq-nvmem`'s
  first `clk_set_rate` on `&kryocc` reprograms the APCS PLL/mux + CBF + vdd-apc
  CPR, and that path aborts. Dropped `operating-points-v2` from all four CPUs so
  cpufreq never scales — cores stay at boot frequency. **This cleared the last
  kernel-side fault.**

**RESULT (verified on hardware 2026-07-27): the kernel boots to completion and
runs `/init`.** Log ends with `Freeing unused kernel memory` → `Run /init as
init process` → the Sarala Rust PID1 executes. The whole NOC/DTS bring-up saga
is done — the mainline kernel is up on marlin.

Two bonus findings:
- **`skip_initramfs` is a non-issue on this path.** The effective cmdline for a
  transient `fastboot boot` keeps our `rdinit=/init` — aboot does *not* append
  `skip_initramfs` / `root=/dev/dm-0`. (Pending task, now moot for `fastboot
  boot`; a flashed boot may differ.)
- The boot.img ramdisk **is Sarala's init**: a 397 KB static aarch64 ELF plus
  busybox and a skeleton rootfs.

### New frontier — userspace: `/init` resets in ~40 ms

After `Run /init` (2.468 s) init runs ~43 ms, then at 2.511 s the kernel emits
one truncated line (only the `[    2.51166` timestamp escapes) and the HTC
watchdog resets. No init output, no full panic text. Signature = **init exits
almost immediately → kernel starts the "Attempted to kill init!" panic →
hardware reset truncates it.** Prime suspect: the ramdisk `/dev` is **empty**
(no `console` node), so Sarala's `console.rs` can't open a console and init
bails. This is the first *userspace* problem — the kernel is no longer the
blocker.

### Remaining next steps

1. **Get Sarala `/init` to survive and log.** Give it a console: mount
   `devtmpfs` on `/dev` early (or ship a static `/dev/console` node in the
   initramfs, and/or `CONFIG_DEVTMPFS_MOUNT`), and make `console.rs` tolerate a
   missing console instead of exiting. First goal: any output from PID1 over the
   jack. Then the stage-1 shell (busybox `sh`).
2. **Trim oneplus-specifics further** — the peripheral/i2c/DVFS trims above are
   the start; continue replacing borrowed `msm8996-oneplus-common` with a
   marlin-specific node set now that we know which nodes matter.
3. **(deferred) `skip_initramfs` on a *flashed* boot** — only relevant if/when we
   flash boot instead of `fastboot boot`; the transient path already runs
   `rdinit=/init`.
