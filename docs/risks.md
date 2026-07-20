# Risk Register

Written at the start, while assessment is still honest. Every entry gets a mitigation or an explicit **Accepted** — no entry is left as a worry without a decision attached.

---

## R1 — Marlin device tree bring-up stalls

**Likelihood:** High · **Impact:** Blocks everything after stage 0

Marlin has never been ported. No `marlin` or `sailfish` device tree exists in the msm8996-mainline staging kernel, and no pmaports device package exists.

**Why this is the good kind of unported.** The SoC-level work — clocks, regulators, interconnects, DPU, Adreno 530, UFS, RPM — is upstream and proven across roughly twenty MSM8996 phones. What remains is *board description*: bounded work with many worked examples. Contrast with the Pixel 3, where the [only porting attempt](https://gitlab.com/postmarketOS/pmaports/-/issues/1229) never got the bootloader to accept an image at all.

**Most likely stall point:** the display panel. Regulator and PMIC rail wiring is the usual culprit — a panel that will not light because a rail it depends on is undeclared or misconfigured.

**Mitigation:**
- Sub-milestone ordering in stage 1 puts serial console before storage before touch before display, so progress is observable and failure is localised.
- Derive board specifics from marlin's downstream Android device tree (Google's kernel source, LineageOS trees).
- Crib structure from `msm8996pro-oneplus3t.dtsi` — same MSM8996 Pro silicon.
- The msm8996-mainline Matrix room is an active community with people who have done exactly this.
- Stage 0 in QEMU means userspace development is never blocked on this.

---

## R2 — Call audio routing consumes months

**Likelihood:** High · **Impact:** Stage 5 stalls indefinitely

The single largest schedule risk in the project. Data and SMS are the achievable 60%; routing the modem's PCM stream through the SoC audio DSP mixer paths via ALSA UCM is where projects like this reliably stall.

**Mitigation:**
- [`msm8996-mainline/alsa-ucm-conf`](https://gitlab.com/msm8996-mainline/alsa-ucm-conf) exists precisely because this is hard; sibling-device mixer paths are the highest-value thing to crib in the entire project.
- Stage 5 is sequenced last among functional stages, so a stall there leaves a genuinely useful device — everything except voice calls.

**Partially accepted.** If call audio proves intractable, Sarala remains a working pocket Linux device with data and SMS. That is a real outcome, not a failure, and saying so now prevents it feeling like one later.

---

## R3 — Camera quality is poor even if the camera works

**Likelihood:** Certain (quality) · **Impact:** Low — camera is a stretch goal, not a stage

> **Revised.** An earlier draft recorded this as "camera realistically never works." That was inherited from the general reputation of Qualcomm camera stacks rather than checked against marlin. Checking it changed the answer, and the correction is recorded rather than silently applied.

**Marlin is unusually well placed.** The mainline [`qcom-camss` driver](https://docs.kernel.org/admin-guide/media/qcom_camss.html) supports exactly two SoC families — MSM8916 and **MSM8996/APQ8096**. The same upstream investment that mainlined the SoC also covers the camera front end: CSIPHY, CSID, ISPIF, VFE.

**The pipeline is viable:**

```
IMX378 sensor → CSIPHY → CSID → ISPIF → VFE     [mainline qcom-camss]
              → raw Bayer
              → debayer + 3A                      [libcamera SoftISP]
              → RGB
```

`qcom-camss` does format conversion, scaling and cropping, and explicitly does **not** do demosaicing, 3A, or denoising — it delivers raw frames. [libcamera's Software ISP](https://fosdem.org/2026/schedule/event/TKSK3G-libcamera-softisp/) fills exactly that gap, and is **explicitly enabled for the `qcom-camss` driver**.

**The real gap is the sensor driver.** Marlin's rear sensor is the Sony **IMX378** (front: IMX179). No mainline `imx378.c` exists, but Raspberry Pi's `imx477` driver handles the IMX378 via chip ID `0x0378` plus a few extra register writes. This is a driver port — bounded work, not research.

**What remains permanently unavailable:**
- **HDR+.** The Pixel XL's camera reputation was computational photography in proprietary userspace. Sarala gets a functioning sensor, not a Pixel camera.
- **PDAF.** The IMX378 supports phase-detect autofocus; SoftISP will not use it. Autofocus will be slow or absent.
- **OIS control.**
- **Power and thermals.** Software debayer of 12 MP frames on 2016 silicon is expensive.

**Accepted as a stretch goal.** Moved from "never" to an optional stage 7 in the roadmap. Not attempted before telephony works, and never load-bearing for the project's success.

---

## R4 — Battery life materially worse than Android

**Likelihood:** Near-certain · **Impact:** Degrades daily usability

Vendor power management tuning — idle state residency, thermal governors, per-rail power sequencing, aggressive suspend paths — represents years of proprietary work that mainline does not inherit. postmarketOS devices consistently report worse battery life than stock Android on identical hardware.

**Mitigation:**
- `msm8996-mainline/cpu-opp-data-msm8996` provides operating point data to build on.
- A ten-process system with no background sync, no Play Services, and no OEM telemetry has a genuine structural advantage on idle drain that partly offsets the missing tuning.

**Partially accepted.** Sarala is unlikely to match stock Android battery life. For a secondary or experimental device this is acceptable; the roadmap does not promise a daily driver.

---

## R5 — Marlin firmware is end-of-life

**Likelihood:** Certain · **Impact:** Permanent unpatched firmware vulnerabilities

The bootloader, modem, DSP, and TrustZone firmware are frozen at their final Google release. No security updates will ever arrive.

**Accepted.** This is inherent to the project's premise of reviving abandoned hardware, not a flaw in the design. Documented in [`threat-model.md`](threat-model.md) alongside the unlocked-bootloader limitation.

---

## R6 — Solo project: motivation decay between stages

**Likelihood:** Moderate · **Impact:** Project abandonment

The most common failure mode for ambitious solo systems projects — not technical defeat, but the gap between stages where nothing visibly works.

**Mitigation:**
- Every stage has an observable exit criterion, so progress is never a matter of judgement.
- Stage 2 (something you drew, on the phone's screen, responding to touch) is deliberately positioned as an early emotional payoff.
- Stage 0 is fully decoupled from hardware, so a stage-1 stall does not halt all forward motion.
- Stages 0–4 are weekend-sized; only stage 5 is a long haul, and it comes after the device is already useful.

---

## R7 — Build loop too slow during device tree iteration

**Likelihood:** Moderate · **Impact:** Stage 1 friction

An i7-9750H cross-compiling kernels is adequate but not fast. Stage 1 involves constant rebuild-and-reboot cycles, which is exactly the workload that first-build cost punishes.

**Mitigation:**
- ccache, incremental builds, and keeping all source on VM-local storage (see [`build-environment.md`](build-environment.md)).
- Device tree changes usually require only a `dtb` rebuild, not a full kernel rebuild — worth setting up a fast path for this specifically.
- **Escape hatch:** a remote build box if the loop becomes the bottleneck. Revisit with real numbers rather than on speculation.

---

## R8 — Scaffolding becomes permanent

**Likelihood:** Moderate · **Impact:** Sarala quietly becomes "busybox with extra steps"

busybox and ModemManager are both introduced as deliberate shortcuts. Shortcuts taken to reach a milestone have a strong tendency to become load-bearing.

**Mitigation:**
- Busybox removal is **stage 3** — before the compositor, not after. Scaffolding removed late is scaffolding never removed.
- Its removal is an exit criterion, not an aspiration.
- The ModemManager → Rust QMI daemon replacement is an explicit step in stage 5 rather than a "someday" note.

---

## R9 — Upstream staging branch churn

**Likelihood:** Low-moderate · **Impact:** Rebase friction

`msm8996-staging` is a moving target carrying patches not yet upstream. Rebasing a marlin device tree across churn could become recurring work.

**Mitigation:**
- Pin to a known-good commit during bring-up; rebase deliberately, never incidentally.
- Keep the marlin DTS as a single self-contained file to minimise conflict surface.
- Upstreaming the port once it works transfers maintenance to the community and is worth doing for that reason alone.
