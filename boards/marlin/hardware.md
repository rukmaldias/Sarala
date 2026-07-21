# marlin — hardware facts from the downstream device tree

The touchscreen controller, panel, and regulator wiring the roadmap listed as
"not yet determined" for stage 1, derived from Google's downstream Android
device tree. **These are downstream facts; the mainline mappings are marked by
confidence** — some are direct, some need verification when the node is
actually brought up.

## Source

LineageOS `android_kernel_google_marlin`, branch `lineage-22.2`, sparse
checkout of the device-tree and config dirs only (~10 MB).

Board file: `arch/arm64/boot/dts/htc/msm8996pro-htc_marlin-a.dts`
— *"HTC Corporation. MSM8996pro + PMI8996 Marlin A"* (the Pixel XL was built by
HTC, so board files live under `dts/htc/`, not `dts/qcom/`).

Include chain (production A revision):

```
msm8996pro-htc_marlin-a.dts
├── ../qcom/msm8996pro.dtsi          SoC
├── ../qcom/msm-pmi8996.dtsi         secondary PMIC
├── ../qcom/msm8996-mtp.dtsi         Qualcomm reference board base
├── msm8996-nc-pins-htc_marlin-xb.dtsi
└── msm8996-htc_marlin.dtsi          the board-specific description (362 lines)
    ├── msm8996-touch-m1.dtsi
    ├── dsi-panel-s1m1-s6e3ha3.dtsi  ← marlin panel (5.5" WQHD)
    ├── dsi-panel-s1m1-ea8064tg.dtsi ← sailfish panel (5.0" FHD), not marlin
    └── dsi-panel-t50.dtsi
```

## Touchscreen — Synaptics

| Property | Downstream value |
|---|---|
| Controller | Synaptics DSX (`synaptics,dsx-i2c`) |
| I²C address | `0x20` |
| IRQ | TLMM GPIO **125**, active-low oneshot (`0x2008`) |
| Bus | BLSP I²C (`qcom,i2c-msm-v2` @ `0x7577000`) |

**Mainline mapping (high confidence):** the Synaptics RMI4 driver,
`syna,rmi4-i2c` (`CONFIG_RMI4_I2C` + `CONFIG_RMI4_F12` for touch). Same address
and IRQ line. Many mainlined MSM8996 siblings use exactly this.

## Display panel — Samsung S6E3HA3

| Property | Value |
|---|---|
| Panel | Samsung **S6E3HA3**, *"M1 WQHD ... 5.5 command mode panel"* |
| Resolution | **1440 × 2560** (WQHD), 720 px per DSI link |
| Interface | **Dual-DSI** — both `mdss_dsi0` and `mdss_dsi1` populated |
| Mode | Command mode (`dsi_cmd_mode`) |
| Refresh | 60 Hz |

**Mainline mapping (medium confidence):** S6E3HA3 is the same Samsung **S6E3**
family as the OnePlus 3T template's S6E3FA3, so
`msm8996pro-oneplus3t-s6e3fa3.dts` is the structural model. Dual-DSI + command
mode is more involved than the OnePlus 3T's single-DSI panel, and a dedicated
`s6e3ha3` panel driver may need writing or adapting from `panel-samsung-s6e3ha2`.
This is the hardest part of the display bring-up.

*The `-a` revision also references `dsi-panel-t50.dtsi`; only S6E3HA3 is wired
active on `mdss_dsi0/1`. Confirm the panel in the actual unit at bring-up —
marlin shipped a panel lottery, which is precisely why the OnePlus 3T template
splits variants across separate `.dts` files.*

## Regulators & GPIOs — panel power (PM8994)

Primary PMIC is **PM8994** (the standard MSM8996 pairing).

| Rail / line | Source |
|---|---|
| `vddio` (panel I/O) | `pm8994_lvs2` |
| `vci` (panel analog) | `pm8994_l29` |
| Reset GPIO | TLMM **39** |
| TE (tear-effect) GPIO | TLMM **10** |
| err-fg GPIO | `pmi8994_gpios 10` |

The `pm8994_*` regulator nodes already exist in the mainline `pm8994.dtsi`, so
wiring the panel is a matter of referencing them, not defining them.

## Storage & serial (still to extract)

UFS and the USB-gadget serial console — stage 1's first two sub-milestones —
are not yet pulled from the downstream tree. UFS on MSM8996 is well mainlined;
the serial console is expected over USB gadget (`console` on the DWC3 gadget)
rather than a physical UART. Extract these next, before writing the panel node.

## Bring-up order (from the roadmap)

Serial console → storage (UFS) → touchscreen → display panel. Each is an
independently observable signal; do not chase the panel until a shell arrives
over serial.
