# Threat Model

Security is one of Sarala's three goals, so this document states plainly both what the design achieves and what it **cannot** achieve on this hardware. A threat model that only lists mitigations is marketing.

---

## Trust boundaries

Ordered most to least trusted:

```
kernel
  └─ init / supervisor          (PID 1, full privilege)
      └─ compositor             (owns input and display)
          └─ applications       (sandboxed, mutually isolated)

    ── trust boundary ──────────────────────────

              modem             (opaque firmware, independent core)
                └─ network      (fully hostile)
```

The heavy line matters more than everything above it. **The modem is not part of Sarala.** It is a separate computer running proprietary firmware on its own core, with its own memory, speaking to a cellular network that anyone can transmit on. It is treated as an untrusted peer that happens to sit inside the same phone case.

---

## The unlocked bootloader — stated plainly

**Marlin cannot do hardware-anchored verified boot.** [`avb_custom_key` support begins with the Pixel 2](https://android.googlesource.com/platform/external/avb/+/pie-release/README.md); the original Pixel XL is not on that list. There is no way to sign Sarala with your own key and re-lock the bootloader.

**Why this is structural, not a gap to be closed.** Marlin shipped with Android 7.1, in the **Verified Boot 1.0** era — a signed boot image plus dm-verity. AVB 2.0, which introduced `vbmeta` and the `avb_custom_key` virtual partition, arrived with Android 8.0 and the Pixel 2. The constraint therefore lives in the **bootloader**, frozen at its 2016 release. No choice made in Sarala's OS design can reach it.

The bootloader stays unlocked permanently. Consequences:

- **An attacker with physical access and a USB cable can flash anything.** No software mitigation changes this.
- **dm-verity still has real value, but a narrower kind than usual.** It detects offline tampering with the rootfs of an installed system and prevents silent modification. It does **not** survive an attacker who simply reflashes, because the root hash's authenticity is anchored in software Sarala controls, not in hardware the attacker cannot rewrite.
- **The boot chain is not attested.** There is no equivalent of a verified-boot warning that a user could rely on.

This is an accepted limitation of the chosen hardware, not an oversight, and it is the one place where Sarala is structurally weaker than stock Android on the same device. It is written down so that no later document quietly implies otherwise.

**Practical consequence for the threat model:** Sarala defends against remote and local-software attackers. It does **not** defend against an attacker with sustained physical possession of the device.

### Do not lock the bootloader

Stated as an operational rule because the reasoning is tempting and wrong.

"Lock it anyway for defence in depth" fails on marlin. With Sarala installed and no custom key available, a locked bootloader verifies against **Google's built-in key**, fails, and lands the device in the [`no valid slot to boot`](https://xdaforums.com/t/solved-custom-rom-install-no-valid-slot-to-boot.4608043/) state.

- **It buys nothing.** There is no key against which the bootloader could validate Sarala. Locking cannot make an unsignable OS verifiable.
- **Recovery is probable but not guaranteed.** It depends on the OEM-unlock flag remaining set and `fastboot flashing unlock` still being honoured from the bootloader. That usually works — but "usually" is carrying too much weight for an irreplaceable device.

The bootloader stays unlocked for the life of the project. This is not a temporary development state to be cleaned up later; it is the permanent, correct configuration for this hardware.

---

## Why the modem is the security priority

The QMI handler parses messages originating from a device Sarala does not control, carrying data influenced by a network anyone can transmit on, in a process that must be privileged enough to configure networking.

That is the exact intersection — **parser + untrusted input + privilege boundary** — where Android concentrated its Rust investment first, and where it saw the largest return. Android moved memory-safety bugs from 76% of its vulnerabilities in 2019 to under 20% in 2025 without rewriting most of the OS, because vulnerability density is not uniform: it concentrates at boundaries like this one.

This is why [`architecture.md`](architecture.md) schedules a Rust QMI daemon to replace ModemManager, and why that rewrite is justified where most rewrites would not be.

**Additional modem containment:**
- No shared memory with the modem beyond what QRTR requires.
- The telephony daemon is sandboxed like any other service — being privileged does not exempt it.
- Modem firmware is loaded from the vendor partition and is unauditable. This is accepted; there is no alternative short of a different phone.

---

## Mitigations available

**Small by construction.** musl, static linking, and a package count in the dozens. Attack surface is a function of how much code exists, and Sarala's principal advantage is having very little.

**Per-service sandboxing.** seccomp filters and Landlock policies, written individually for each service. This is only tractable because there are roughly ten processes — it is never tractable on a general-purpose distribution, and it is the clearest case of minimalism and security compounding rather than trading off.

**No suid binaries at all.** Capabilities where privilege is genuinely needed. The suid mechanism is a persistent source of privilege escalation and Sarala has no reason to carry it.

**Compile-time hardening,** applied uniformly: RELRO, stack protector, `_FORTIFY_SOURCE`, PIE.

**Hardened allocator.** See the layering note below.

**Read-only rootfs with dm-verity,** mutable state confined to a data partition — with the caveat recorded above.

**Wayland's structural isolation.** Applications cannot read each other's input or capture each other's surfaces. X11 could never provide this; any X client can read every keystroke in the session. This is isolation by architecture rather than by policy.

**Memory-safe language at every boundary.** Init, compositor, telephony daemon, and all application parsers — vCard, MIME, images — are Rust.

---

## Mitigations unavailable

**`avb_custom_key`** — Pixel 2 and later only, as above.

**Arm MTE (Memory Tagging Extension)** — requires ARMv8.5+. Marlin is ARMv8.0. Google pairs Rust with MTE on modern devices; Sarala gets only half of that pairing, so **software mitigations carry more weight here than they would on current silicon.**

**Firmware updates** — marlin is long past end-of-life. Firmware-level vulnerabilities are permanent. No mitigation exists; this is a property of choosing abandoned hardware and is accepted as the cost of the project's premise.

---

## The `unsafe` audit surface

Rust's guarantees end at `unsafe`. Every `unsafe` block in Sarala is a reviewable item, and they cluster predictably: FFI to C libraries, DRM/KMS ioctls, and raw memory mapping.

The reference lesson is Android's first near-miss Rust vulnerability — a buffer overflow inside an `unsafe` block in an image parser, **rendered non-exploitable by a hardened allocator** rather than by the language. Rust and runtime hardening are complementary layers, not substitutes. Sarala therefore keeps seccomp, Landlock, and a hardened allocator around Rust processes exactly as it would around C ones.

**Practice:** `unsafe` blocks carry a comment stating the invariant being upheld and why it holds. A block without that comment is a bug regardless of whether it currently misbehaves.

---

## What this model does not cover

- **Physical attackers**, per the bootloader section.
- **Supply chain** — Rust crate dependencies are trusted implicitly today. Worth revisiting when the dependency count grows past what one person can review.
- **Side channels** — out of scope.
- **The baseband's own security** — unauditable, unpatched, and permanent. Containment is the only available strategy.
