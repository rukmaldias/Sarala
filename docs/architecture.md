# Architecture

Every layer below states three things: what is **borrowed**, what is **written**, and **why the line falls there**. The line is the whole design.

---

## 0. The organising constraint

An old ThinkPad runs a generic Linux kernel because PC hardware is discoverable: UEFI, ACPI, PCIe, class-compliant USB, a VESA framebuffer as a floor. You can boot a kernel on hardware it has never seen.

A phone is a bespoke embedded system. There is no discovery mechanism — hardware topology is described by a hand-written device tree, the boot chain is a vendor's signed proprietary stack, and the interesting peripherals are driven by vendor code and firmware blobs.

So the question is never "which kernel?" It is **"who writes and maintains the drivers for this SoC?"** Sarala answers that by borrowing the answer, and spending its effort everywhere else.

---

## 1. Boot chain — entirely borrowed

Marlin boots the way Google designed it to, and this is inherited rather than chosen:

```
Qualcomm PBL (masked ROM)
  → XBL / SBL (Qualcomm-signed, immutable)
    → ABL / LK (bootloader, unlockable)
      → boot.img  ← Sarala's entry point
```

Sarala's kernel and initramfs are packed into an Android `boot.img` via `mkbootimg` and handed to the device over fastboot. Even a non-Android OS on a phone boots through the Android boot path. Understanding `boot.img` layout, `dtbo`, and A/B slots is not a compromise — it is core mobile-systems knowledge.

**Preserved partitions.** Marlin is an A/B device. The `modem`, `wifi`/`bt`, `dsp`/`adsp`, and Qualcomm firmware partitions must remain intact — the mainline kernel loads firmware *from them* at runtime via `remoteproc`. Sarala never touches them. Wiping them is one of the few genuinely unrecoverable mistakes available here.

**`fastboot boot`, never `fastboot flash`.** Throughout bring-up, images are booted transiently. The device's existing Android install stays on disk as a known-good fallback. This is the standing safety net and it should survive into late stages.

---

## 2. Kernel — borrowed, configured, and extended by one file

**Borrowed:** mainline Linux, tracking the [`msm8996-staging`](https://gitlab.com/msm8996-mainline/linux) branch. This carries SoC support that is upstream or nearly so: clocks, regulators, interconnects, the MDSS/DPU display controller, Adreno 530 via `msm`, UFS, RPM, and the QRTR/GLINK transports used to reach the modem and DSPs.

**Written:** `msm8996pro-google-marlin.dts` — the device tree. This is the entirety of Sarala's kernel-source contribution, and it is enough. A device tree describes *this board*: which regulator rail feeds which peripheral, which GPIO is the touchscreen reset, which panel is attached and on which DSI lane configuration, how much RAM and where the reserved firmware regions sit.

**Why the line falls there.** The expensive, multi-year work is SoC enablement, and it is done. Board description is bounded work with ~20 worked examples to learn from. Writing a driver later — for one peripheral, possibly in Rust — is a reasonable stretch goal. Rewriting the driver ecosystem is an engineer-decades problem and is not attempted.

**Configuration is part of the work.** The kernel config is stripped toward the minimum this device needs. This is not just size reduction; reading the config option by option is how you learn what the hardware actually requires.

---

## 3. Init and service supervision — written, in Rust

**Written:** PID 1.

Responsibilities: mount the early filesystems (`/proc`, `/sys`, `/dev`, `/run`), start services in dependency order, supervise and restart them, reap orphaned children, handle signals, and shut down cleanly.

**Why Rust here first.** PID 1 dying is a kernel panic. There is no process in the system where a memory-safety guarantee is worth more. It is also small enough — a few hundred lines for a first version — that writing it teaches process supervision, signal handling, and the `SIGCHLD`/`waitpid` reaping contract without drowning you.

**Explicitly not systemd.** Wrong fit for a ten-process system, and the point is to learn what an init actually does.

**Borrowed, temporarily:** busybox, for a shell and coreutils, so that stage 0 reaches a prompt quickly. Busybox is scaffolding with a scheduled demolition date — see stage 3 in the roadmap. Its last job is being deleted.

---

## 4. Graphics — borrowed driver, written everything else

**Borrowed:** the kernel `msm` DRM/KMS driver and Mesa's **freedreno** Gallium driver. The Adreno is the best-supported mobile GPU in open source precisely because it was reverse-engineered thoroughly. On marlin this means GPU acceleration with **no proprietary blob** — a genuinely rare property.

**Written:** everything above the DRM device node.

- **First:** direct DRM/KMS rendering. Open the card, find the connector and CRTC, allocate a dumb buffer, page-flip. `kmscube` is the reference. This is the graphics equivalent of writing your own PID 1 — unglamorous and clarifying.
- **Then:** a Wayland compositor built on **Smithay**, the mature Rust compositor library. Touch-driven, gesture-based app switching, no window decorations, no desktop metaphor.

**No X11.** Beyond being wrong for touch, X11 cannot isolate input between clients — any X client can read every keystroke. Wayland gives input and screen isolation as a structural property, which the threat model leans on.

---

## 5. Telephony — the boss fight

The modem is **a separate computer**. It runs its own proprietary firmware on its own core, has its own memory, and speaks to the cellular network independently of the application processor. The kernel loads its firmware via `remoteproc` and communicates over **QRTR** (Qualcomm IPC Router), carrying **QMI** messages.

Two paths, and the recommendation is to walk both in order:

**Pragmatic first — borrow ModemManager.** A mature C daemon that speaks QMI and exposes a clean D-Bus API. Sarala's dialer and SMS applications are written against that API. This gets to a working phone fastest.

**Then replace it — write a Rust QMI daemon.** Using `libqmi`'s protocol definitions as reference, speak QMI directly. This is the single best-justified rewrite in the project, for the reason set out in the threat model: the modem is an untrusted peer, and a QMI handler is a parser of attacker-influenced input sitting at a privilege boundary. That is precisely the category where Android deployed Rust first and got the largest return.

**The honest warning.** Data connection and SMS are the achievable 60%. **Call audio routing is the part that eats months** — wiring the modem's PCM stream through the SoC's audio DSP mixer paths via ALSA UCM. The [`msm8996-mainline/alsa-ucm-conf`](https://gitlab.com/msm8996-mainline/alsa-ucm-conf) repository exists precisely because this is hard, and its mixer paths for sibling devices are the most valuable thing to crib in this entire project.

---

## 6. Applications — written, in Rust

Four applications, in the order they should be built:

| App | Core work | Notes |
|---|---|---|
| **Contacts** | vCard parsing, SQLite, touch UI | The right first app — no external dependencies beyond storage |
| **Dialer** | D-Bus to the telephony daemon | Trivial once telephony works; useless before |
| **Email** | IMAP, MIME parsing, rendering | MIME is a classic exploitation surface; `mail-parser` handles it in safe Rust |
| **Maps** | Offline tile rendering | Hardest of the four. Prefer embedding Organic Maps' rendering core over writing tile rendering from scratch |

**UI toolkit:** Slint — designed for embedded, Rust-native, small. Keeps the entire application stack in one language.

---

## 7. The Rust/C boundary

Sarala is greenfield, which means it avoids the tax that dominates Android's Rust migration: interop with thirty million lines of legacy C++. Google's policy is "new code in Rust, leave old code alone." Sarala can invert it — **Rust by default, C only where ecosystem gravity makes fighting pointless.**

| Component | Language | Why |
|---|---|---|
| Kernel | C (borrowed) | Cannot be rewritten by one person |
| Mesa / freedreno | C (borrowed) | Enormous, works, open, actively maintained |
| ALSA plumbing | C (borrowed) | Configuration-heavy; the value is in UCM files, not code |
| ModemManager | C (borrowed, then replaced) | Stepping stone to the Rust QMI daemon |
| **PID 1 / supervisor** | **Rust** | Highest-consequence failure in the system |
| **Compositor** | **Rust** | Smithay is mature; sits between apps and input |
| **QMI daemon** | **Rust** | Parser of untrusted input at a privilege boundary |
| **All applications** | **Rust** | vCard, MIME, and image parsing are classic memory-safety sinks |

**Sequencing by security return.** Android moved memory-safety vulnerabilities from 76% of its total in 2019 to under 20% in 2025 without rewriting most of the OS, because the return is not uniform across a codebase — **it concentrates at trust boundaries.** Sarala is small enough to be Rust nearly everywhere, but building in the order *init → compositor → modem interface → parsers* means every early hour buys the most.

**Rust is not a substitute for runtime hardening.** Android's first near-miss Rust vulnerability was a buffer overflow in an `unsafe` block, rendered non-exploitable by a hardened allocator. The layers are complementary. Every `unsafe` block in Sarala is an audit item, and the hardening described in the threat model applies to Rust processes too.
