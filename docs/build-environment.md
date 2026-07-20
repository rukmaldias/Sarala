# Build Environment

## Host reality

Development host is an **Intel MacBook Pro** — i7-9750H (6 cores / 12 threads), 16 GB RAM, macOS. This single fact determines the entire setup, so it is recorded here rather than left implicit.

**macOS Hypervisor.framework accelerates x86_64 guests only.** On an Intel host, an aarch64 guest runs under QEMU's TCG interpreter — full software emulation, roughly an order of magnitude slower. That is fine for booting a shell and unusable for compiling a kernel.

Therefore: **an x86_64 Linux guest, cross-compiling to aarch64.**

> **On Apple Silicon the opposite choice would be correct** — an aarch64 guest would run natively under HVF and compile for the target without cross-compilation at all. If this project ever moves to an ARM Mac, revisit this decision. It is host-specific, not a principle.

---

## Choice of guest distribution

**Debian stable.**

The distribution matters less than it appears, because the two things that dominate this build are distribution-independent:

- **Rust comes from `rustup`**, not from the distribution. This neutralises package currency as a criterion for the one component where currency genuinely matters.
- **The kernel build is undemanding.** Mainline Linux compiles with long-obsolete GCC versions. No actively maintained distribution is too old.

That leaves two criteria that do discriminate, and both favour Debian:

**Cross-compilation is Debian's home ground.** Multiarch exists specifically to build for one architecture on another, and Debian cross-builds its own distribution packages with it. Other distributions ship working cross-toolchains as a convenience; in Debian it is load-bearing infrastructure.

**A build host should be boring.** Rolling release is a virtue on a daily driver and a liability on a toolchain depended upon for months. This project explicitly anticipates gaps between stages (see [`risks.md`](risks.md) R6) — returning after a long break should mean `apt update` and security fixes, not an unavoidable full-system upgrade that may move GCC underneath a half-finished device tree.

**Rejected alternatives, and why:**

| Option | Why not |
|---|---|
| **Arch** | Best documentation available anywhere and the AUR packages every Android tool. But rolling churn on a build host revisited after long gaps is real friction, and partial upgrades are unsupported, so the risky upgrade cannot be declined. |
| **Ubuntu LTS** | A defensible near-tie with Debian. Rejected only for preferring a smaller base; if third-party embedded tooling ever assumes Ubuntu, switching costs nothing. |
| **Alpine** | Philosophically closest to Sarala itself, and pmbootstrap's native home. But a musl host adds friction when cross-building glibc-targeting components, and pmbootstrap runs anywhere — so the philosophical alignment buys nothing concrete. |

---

## Guest VM

**x86_64 Debian stable**, run under QEMU with HVF acceleration.

| Setting | Value | Reasoning |
|---|---|---|
| Accelerator | `-accel hvf` | Non-negotiable; see verification below |
| vCPU | 8 | Host is 6c/12t — leave headroom for macOS |
| RAM | 8 GB | Half the host; kernel builds are not memory-hungry |
| Disk | ≥ 60 GB | Kernel trees, ccache, and cargo registry grow fast |

---

## Toolchain

**Kernel:** `gcc-aarch64-linux-gnu` from the Debian repositories.

**Rust:** install via `rustup`, then `rustup target add aarch64-unknown-linux-musl`.

Use `rustup` rather than the distro Rust package, so toolchain currency never depends on packaging lag. musl rather than glibc for static linking and a smaller attack surface.

**Also needed:** `bison`, `flex`, `bc`, `openssl`, `ccache`, `dtc` (device tree compiler), and `mkbootimg` (from AOSP tooling or the standalone Python implementation).

---

## Two distinct QEMU roles — do not conflate them

This is the most common source of confusion in this setup.

| Role | Invocation | Speed | Purpose |
|---|---|---|---|
| **Build host** | `qemu-system-x86_64 -accel hvf` | Near-native | Compiling |
| **Stage-0 target** | `qemu-system-aarch64 -M virt` | TCG-emulated | Booting Sarala |

The target being emulated is acceptable — it boots a small initramfs and runs a shell. It never compiles anything.

**Run the target on the macOS host, not nested inside the build VM.** macOS does not expose nested virtualisation, so nesting adds overhead for no benefit.

---

## Artifact flow

```
[Debian VM] build kernel + initramfs → boot.img
    ↓  virtiofs / 9p / scp
[macOS]    fastboot boot boot.img
    ↓  USB
[Pixel XL]
```

**Flashing tools run natively on macOS** — `brew install android-platform-tools`. USB passthrough into a QEMU VM on macOS is fragile and entirely unnecessary; only the finished image needs to cross the boundary.

**`fastboot boot`, never `fastboot flash`.** The image is booted transiently and the device's Android install stays intact as a fallback. This is the standing safety net for all of stage 1.

---

## pmbootstrap — read it, don't adopt it

[`pmbootstrap`](https://wiki.postmarketos.org/wiki/Pmbootstrap) is postmarketOS's build tool, and it is the reference implementation of **precisely** Sarala's stage-1 workflow: chroot management, cross-compilation, kernel packaging, and `boot.img` assembly for new device ports. It is distribution-independent — it manages its own chroots — so this is not a guest-OS consideration.

**Decision: study it, do not adopt it.** Building Sarala's own tooling is part of the point; outsourcing it to pmbootstrap would skip the learning that stage 0 and stage 1 exist to deliver.

But read its source before writing that tooling. Specifically worth studying:

- How it structures cross-compilation chroots
- Its kernel packaging conventions, and the `deviceinfo` format describing a device port
- Its `boot.img` assembly and `mkbootimg` invocation
- Its device-port directory layout — a proven answer to a problem Sarala has too

Reinventing these deliberately is learning. Reinventing them *unaware that solved answers exist* is just slower.

**Escape hatch:** if stage 1 stalls badly on packaging mechanics rather than on the device tree itself, adopting pmbootstrap temporarily is a reasonable unblock. The device tree is the learning objective; the image packing around it is not.

---

## Performance rules

The difference between a fast setup and a miserable one is almost entirely these four points.

**1. Never put the kernel tree on a macOS bind mount.**
Two independent reasons, either one fatal:
- macOS filesystems are case-insensitive by default, and the kernel source contains filenames that differ only in case.
- Cross-boundary mount I/O (virtiofs/9p) dominates build time for workloads with many small files, which describes a kernel build precisely.

Source lives on the VM's own virtual disk. Only finished artifacts cross the boundary. **This is the single most common cause of "my VM is slow."**

**2. Verify HVF is actually engaged.**
QEMU can silently fall back to TCG — for example if the accelerator is misspelled or unavailable. Near-native becomes unusable and it is easy not to notice. Check `-accel hvf` is accepted at startup and sanity-check that a trivial build completes in seconds rather than minutes.

**3. Parallelise to the VM's allocation.**
`make -j8` with 8 vCPU. Going wider than the host's real core count is counterproductive.

**4. Cache aggressively.**
`ccache` for kernel rebuilds; keep the cargo registry and kernel source tree on persistent VM storage so incremental rebuilds stay incremental.

---

## Expectations, honestly

First full kernel build on this hardware: **tens of minutes.** Incremental rebuilds with ccache: fast enough not to think about.

That first-build cost is acceptable for stages 0 and 2–6, where kernel rebuilds are rare. **Stage 1 is the exception** — device tree iteration means rebuilding and rebooting constantly. If that loop becomes the bottleneck, a remote build box (a cheap ARM64 or larger x86 cloud instance) is the escape hatch.

Revisit that then, with real numbers. Not now, on speculation.
