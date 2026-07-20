# Roadmap

Each stage has an **exit criterion that can be observed, not judged.** "The compositor is working" is not an exit criterion. "Two applications, switchable by touch gesture" is.

Stages 0–4 are weekend-sized. Stage 5 is the long haul. This asymmetry is real and worth internalising before starting.

---

## Stage 0 — Build host and first boot

Build the development environment, then boot your own PID 1 under emulation.

1. Provision the x86_64 Debian stable VM under QEMU/HVF — see [`build-environment.md`](build-environment.md).
2. Install the aarch64 cross-toolchain and `rustup` target.
3. Build the `msm8996-staging` kernel for `aarch64` with a `virt`-compatible config.
4. Cross-compile a static busybox, or use a prebuilt static aarch64 binary.
5. Write PID 1 in Rust: mount `/proc`, `/sys`, `/dev`; spawn a shell; reap orphans; handle `SIGCHLD` and `SIGTERM`.
6. Pack kernel + initramfs and boot on `qemu-system-aarch64 -M virt`.

**Exit criterion:** a PID 1 you wrote hands you a shell prompt in QEMU.

*Why this first:* it decouples "does my userspace work" from "does my device tree work." Every later stage benefits from being able to test userspace changes in seconds without touching hardware.

---

## Stage 1 — Marlin bring-up

Write the device tree. This is the hardest *unknown* in the project, and the best learning artifact in it.

**Primary reference:** `msm8996pro-oneplus3t.dtsi`. The OnePlus 3T is the same MSM8996 **Pro** silicon as marlin, making it the closest structural template available. Note that it splits panel variants across `msm8996pro-oneplus3t-s6e3fa3.dts` and `-s6e3fa5.dts` — marlin's Samsung AMOLED panel needs exactly this pattern.

**Secondary references:** `msm8996-xiaomi-common.dtsi`, `msm8996-sony-xperia-tone.dtsi`, `msm8996-oneplus3.dts`. Reading several is more useful than reading one — the differences show you what is board-specific.

Work in this order, because each step is independently observable and each depends on the last:

| Sub-milestone | Observable signal |
|---|---|
| **Serial console** | Kernel log over USB gadget serial |
| **Storage** | UFS enumerates; you can mount a partition |
| **Touchscreen** | `evtest` reports touch coordinates |
| **Display panel** | Backlight on, then a test pattern via DRM |

Marlin's exact touchscreen controller, panel model, and regulator wiring are **not yet determined** — deriving them from the downstream Android device tree (available in Google's kernel source and LineageOS trees for marlin) is part of the work.

**Exit criterion:** `fastboot boot` a Sarala image on the Pixel XL and get a shell over USB gadget serial.

*Nothing is flashed. The Android install remains intact throughout.*

---

## Stage 2 — Display

1. Direct DRM/KMS rendering — enumerate connector and CRTC, allocate a buffer, page-flip.
2. `libinput` for touch events.

**Exit criterion:** something you drew, on the phone's screen, responding to your finger.

*This is the emotional midpoint of the project. Everything before is text on a serial console.*

---

## Stage 3 — Userspace ownership

Demolish the scaffolding. This stage exists to keep the busybox dependency from quietly becoming permanent.

1. Replace busybox coreutils with **uutils** (Rust coreutils).
2. Mature PID 1 into a real supervisor: dependency ordering, restart policy, service state tracking, log handling.

**Exit criterion:** the busybox package is removed from the image and the system still boots to a usable shell.

*Deliberately placed before the compositor, not after. Scaffolding removed late is scaffolding never removed.*

---

## Stage 4 — Compositor and first application

Rust-native from birth — nothing here inherits from the busybox era.

1. Smithay-based Wayland compositor.
2. Touch gestures for application switching.
3. Contacts application: vCard parsing, SQLite storage, Slint UI.

**Exit criterion:** two applications running, switchable by touch gesture.

---

## Stage 5 — Telephony

The long haul. Budget accordingly and expect this stage alone to exceed all prior stages combined.

1. **Modem boot.** `remoteproc` loads modem firmware; QRTR endpoints appear.
2. **Data.** ModemManager brings up a data connection.
3. **SMS.** Send and receive.
4. **Call audio.** Route the modem PCM stream through the audio DSP mixer paths via ALSA UCM. Crib aggressively from [`msm8996-mainline/alsa-ucm-conf`](https://gitlab.com/msm8996-mainline/alsa-ucm-conf).
5. **Dialer application** against the telephony D-Bus API.
6. **Rust QMI daemon** replacing ModemManager — the security-motivated rewrite.

**Exit criterion:** a phone call, with audio, both directions.

*Steps 1–3 are the achievable 60%. Step 4 is where projects like this stall. See [`risks.md`](risks.md).*

---

## Stage 6 — Hardening

Deferred to last deliberately: hardening a system whose shape is still changing wastes effort and obscures failures.

1. Read-only rootfs with **dm-verity**; mutable state confined to a data partition.
2. Zero suid binaries; capabilities where genuinely needed.
3. Per-service **seccomp** and **Landlock** policies — tractable at ten processes.
4. Full hardening compile flags: RELRO, stack protector, `_FORTIFY_SOURCE`, PIE.
5. Hardened allocator.
6. Audit every `unsafe` block in the Rust codebase.

**Exit criterion:** rootfs verifies under dm-verity, every service runs under an explicit sandbox policy, and no suid binary exists on the image.

---

## Stage 7 (optional) — Camera

A genuine stretch goal, not a commitment. Attempt only after stage 5, and only if the appetite is there.

Marlin is better placed for this than most phones: mainline [`qcom-camss`](https://docs.kernel.org/admin-guide/media/qcom_camss.html) supports MSM8996, and [libcamera's Software ISP](https://fosdem.org/2026/schedule/event/TKSK3G-libcamera-softisp/) — which supplies the demosaic and 3A that `qcom-camss` deliberately omits — is explicitly enabled for that driver.

1. Port a sensor driver for the Sony **IMX378** (rear). Raspberry Pi's `imx477` driver already handles it by chip ID `0x0378`; upstreaming a proper `imx378` driver is the clean version.
2. Wire the CAMSS media graph in the marlin device tree.
3. Bring up libcamera with SoftISP.
4. Capture application.

**Exit criterion:** a photograph, taken on the phone, saved to disk.

**Expect poor image quality.** No HDR+, no PDAF, no OIS — see [`risks.md`](risks.md) R3. The goal is a working camera, not a good one.

---

## What is not on this roadmap

**Hardware-anchored verified boot.** Structurally unavailable on marlin, and attempting it is dangerous — see [`threat-model.md`](threat-model.md).

**Android app compatibility.** Never a goal.

**Good photographs.** Distinct from "a camera" — see stage 7.
