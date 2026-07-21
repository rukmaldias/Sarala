# marlin — serial console access

How to physically read marlin's debug console. This is the linchpin of stage 1:
without it, bring-up is blind. With it, `earlycon` output is readable and every
later peripheral (USB, UFS, panel) becomes ordinary debugging instead of
guesswork.

## The console is on the 3.5mm headphone jack — not USB-C

The Pixel XL (2016) kept its headphone jack, and marlin exposes the debug UART
there, the classic Nexus/Pixel scheme. (The Pixel 2 dropped the jack and moved
UART to the USB-C SBU pins at 1.8V — a *different* method that does not apply
here.) A plain USB-C cable cannot read marlin's console at all; USB-C is for
fastboot / adb / USB-gadget only.

Confirmed working on marlin by jpuderer (bootloader *and* kernel output).

| Property | Value |
|---|---|
| Port | 3.5mm TRRS headphone jack |
| Level | **3.3V** (not 1.8V — that is the Pixel 2) |
| Line | 115200 8N1, no flow control |
| UART | `blsp1_uart2` @ `0x7570000` (mainline `ttyMSM0`; downstream `ttyHSL0`) |

## Enabling it

`fastboot oem uart enable`, with the bootloader unlocked and the device in
fastboot mode. **Verified on this unit** (`HT69A0202791`): the command returns
`OKAY`, so this bootloader version supports UART routing. (`fastboot oem uart
disable` reverses it. These OEM commands are bootloader-version dependent.)

## The cable (~$20, DIY)

There is **no reliable off-the-shelf product** — it is a small DIY build. The
build below is confirmed on a Pixel XL by jpuderer (bootloader + kernel output).

Bill of materials:

- **SparkFun FTDI Basic Breakout — 3.3V** — the USB-UART adapter.
  <https://www.sparkfun.com/sparkfun-ftdi-basic-breakout-3-3v.html>
- **TRRS 3.5mm 4-pole audio pigtail** (~18").
- **Jumper wires** (M/M).
- **USB mini-B cable** — FTDI board to the Mac.

Build guide (this page is effectively the shopping list, with a wiring
diagram): <https://www.jpuderer.net/2018/02/pixel-debug-cable.html>

TRRS pinout (CTIA): **Tip = TX, Ring1 = RX, Ring2 = GND, Sleeve = mic/detect
(activation)**. Verify the physical wire↔pin mapping with a multimeter — TRRS
cable colours are not standard, and getting this wrong is the usual failure.

### Signal-level caveat

The activation on the sleeve wants ~3.3 V, but the UART **TX/RX signals are
actually 1.8 V**. The simple 3.3 V FTDI build above works on marlin anyway (the
1.8 V signal reads fine, and it is the proven build), so it is the pragmatic
choice. The electrically *correct* alternative is a 1.8 V FTDI cable
(`TTL-232RG-VREG1V8-WE`) with a 33 Ω resistor rerouting its 3V3OUT to the mic
sleeve — cleaner but a less common cable and a solder mod. Reference:
<http://www.pabr.org/consolejack/consolejack.en.html>

Recommendation: the 3.3 V FTDI build. Cheaper, no solder strictly required
(header pins + bare TRRS leads joined by jumpers), confirmed on this device.

## How this connects to the boot image

`earlycon=msm_serial_dm,0x7570000` in the cmdline (see
[`boot-image.md`](boot-image.md)) drives this exact UART, before any driver or
regulator probes. With `oem uart enable` set and the cable attached, that is the
first output to watch for after `fastboot boot`.

**One nuance to watch at bring-up:** `oem uart enable` sets the bootloader-level
mux that routes `blsp1_uart2` to the jack. Bootloader and early-kernel
(earlycon) output is confirmed to reach it. If output stops once the mainline
kernel takes over the pinmux, the kernel may be re-muxing those pins — a dts
pinctrl detail to check, not a wiring fault.

## Status of the port

No mainline marlin/sailfish port exists to crib from — verified against the
msm8996-mainline staging pmaports (has crosshatch and sargo, no marlin). This is
a first port.
