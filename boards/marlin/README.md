# marlin

Google Pixel XL — MSM8996 Pro / Snapdragon 821.

Empty until stage 1. The device tree lands here, alongside whatever
Sarala's equivalent of pmbootstrap's `deviceinfo` turns out to be.

Primary structural reference is `msm8996pro-oneplus3t.dtsi` — same MSM8996
**Pro** silicon, and it already splits panel variants across separate `.dts`
files, which is the pattern marlin's Samsung AMOLED panel needs. See
[`../../docs/roadmap.md`](../../docs/roadmap.md) stage 1.

Marlin's touchscreen controller, panel model and regulator wiring are not yet
determined; deriving them from the downstream Android device tree is part of
the work.
