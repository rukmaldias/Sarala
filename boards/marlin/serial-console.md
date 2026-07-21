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

## The cable (~$15–20)

- A **3.3V** USB-UART adapter — e.g. SparkFun FTDI Basic 3.3V (a 1.8V adapter
  will not read marlin; that is the Pixel 2).
- A TRRS (4-pole) audio pigtail.
- Jumper wires; verify pin↔wire mapping with a multimeter — TRRS wire colours
  are not standard.

Pre-made "Pixel debug cables" also exist. Either way the signal is 3.3V UART on
the jack, TX/RX/GND.

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
