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
| UART | **`blsp2_uart2` @ `0x75b0000`** (the console/earlycon `msm_serial_dm0`) |

> **Correction (2026-07-27):** the debug UART on the jack is **`blsp2_uart2`
> @ `0x75b0000`**, *not* `blsp1_uart2` @ `0x7570000` as originally recorded.
> Proven during bring-up by tracing `port->mapbase`: the console output that
> reaches the jack is written to `0x75b0000` (blsp2), while writes to
> `ttyMSM0`/blsp1 (`0x7570000`) never appear. The initial `ttyHSL0 → 0x7570000`
> mapping was an incorrect inference. See [`first-boot.md`](first-boot.md).

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

## Confirmed working build (verified on this unit, 2026-07-23)

Console output was achieved — the running kernel's `dmesg` (msm_pcie,
cnss_wlan_pci, binder, battery charger) streamed live at 115200. Both bootloader
*and* kernel output reach the jack. What actually worked, and the traps that
cost hours:

**Adapter used:** a generic **FT232RL USB-to-TTL board, USB-C** (silkscreen
`V1350 / YP-05`), *not* the SparkFun board in the BOM above — functionally
identical, same FTDI-Basic pinout. It has a **3.3 V / 5 V voltage-select jumper
that must be on 3.3 V** (the SparkFun board is fixed-3.3 V and can't get this
wrong). Note the USB-C connector supersedes the "USB mini-B cable" BOM line for
this board.

**Verified wire → signal mapping** (this specific TRRS pigtail; colours are
*not* transferable to another cable):

| Wire  | Signal | Plug contact |
|-------|--------|--------------|
| Red   | RXI    | Tip (phone TX)  |
| White | TXO    | Ring1 (phone RX) |
| Green | GND    | Ring2 |
| Black | 3V3    | Sleeve (activation) |

**Board pin layout** — always wire by the **silkscreen label, never by pin
position**; the board's orientation flips the left↔right numbering but never the
labels. Holding this board pins-up / USB-C-down, left→right:

`GND · CTS · 3V3 · TXO · RXI · DTR` — so GND and 3V3 straddle CTS, and TXO/RXI
are adjacent (which makes a TX/RX swap easy). Leave CTS and DTR empty.

### Two traps that ate most of the bring-up session

1. **macOS: use `/dev/cu.usbserial-*`, not `/dev/tty.usbserial-*`.** The `tty.`
   node waits for carrier-detect (DCD) and just hangs / returns
   `[screen is terminating]` with an FTDI; `cu.` (call-up) opens immediately.
   This one masqueraded as a dead cable for a long time. Command:
   `screen /dev/cu.usbserial-3 115200`.
2. **Prove the adapter before blaming the cable — loopback test.** Short the
   board's **RXI ↔ TXO** pins (jumper or paperclip), open `screen`, and type: a
   working adapter echoes the characters straight back (screen does not
   locally-echo, so what you see is the loop). Isolates Mac/driver/port faults
   from cable/jack faults in 30 seconds. Do this first, every time.

Cable colours were wrong on *both* pairs versus the initial guess — exactly the
"colours are not standard, verify" failure. With no multimeter, brute-forcing
the 4 pair-swap combinations found it; a multimeter would have been faster
(there are 24 possible wire→contact orderings, and the 4-combo shortcut only
works if the signal-pair vs ground/sleeve-pair grouping happens to be guessed
right).

## How this connects to the boot image

Use **bare `earlycon`** in the cmdline (see [`boot-image.md`](boot-image.md)),
*not* `earlycon=msm_serial_dm,0x75b0000`. Mainline `msm_serial.c` registers only
`OF_EARLYCON_DECLARE(msm_serial_dm, "qcom,msm-uartdm", …)` — matched via the DT
`stdout-path` (`serial1` → `serial@75b0000`, blsp2), not by the named `earlycon=`
form (there is no bare `EARLYCON_DECLARE`). Bare `earlycon` therefore drives this
exact UART before any driver or regulator probes.

**Set the real console to this same UART: `console=ttyMSM1,115200n8`.** ttyMSM1 is
`blsp2_uart2` (the jack). This is not optional — if the runtime console is any
*other* UART, serial_core powers `blsp2_uart2` off as a non-console port and the
still-live earlycon's status-register poll into the gated block triggers an
`RPM:TZ ABORT` reset (see [`first-boot.md`](first-boot.md)). As the real console
it is never powered off. **Confirmed 2026-07-28:** with
`earlycon console=ttyMSM1,115200n8`, output reaches the jack through the full
boot and lands on the Sarala `/bin/sh` prompt.

**One nuance:** `oem uart enable` sets the bootloader-level mux that routes the
debug UART (blsp2) to the jack. Bootloader, earlycon, and the runtime ttyMSM1
console are all confirmed to reach it.

## Status of the port

No mainline marlin/sailfish port exists to crib from — verified against the
msm8996-mainline staging pmaports (has crosshatch and sargo, no marlin). This is
a first port.
