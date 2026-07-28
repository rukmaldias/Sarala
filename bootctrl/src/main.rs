//! bootctrl — mark an A/B boot slot "successful" in the Qualcomm GPT attributes.
//!
//! marlin stores its A/B slot metadata in the top byte (bits 48–55) of each
//! boot_{a,b} GPT partition entry's 64-bit attribute field:
//!
//!   bits 48–49  priority (0–3)
//!   bit  50     active
//!   bits 51–53  retry count remaining (0–7)
//!   bit  54     successful
//!   bit  55     unbootable
//!
//! aboot decrements the retry count on every boot that is not confirmed; when it
//! hits 0 it marks the slot unbootable and rolls back to the other slot. Android
//! normally sets "successful" at runtime (its bootctrl HAL). Sarala has no such
//! HAL, so this tool does the same job: set the active slot successful + full
//! priority/retries so the bootloader stops rolling back.
//!
//! Usage:
//!   bootctrl dump [disk]                 read-only: print both GPTs + slots
//!   bootctrl mark [disk] [slot] [--commit]
//!       plan (and, with --commit, perform) marking <slot> successful.
//!       disk defaults to /dev/sda; slot defaults to "auto" (the androidboot
//!       slot_suffix from /proc/cmdline, e.g. boot_b). Without --commit it is a
//!       dry run that prints the before/after attributes and writes nothing.
//!
//! The write updates the partition entry, recomputes the entry-array CRC32 and
//! the header CRC32, and rewrites BOTH the primary and backup GPT so they stay
//! consistent (the kernel already warns marlin's primary entry-array CRC is off
//! and falls back to the backup).

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::process::ExitCode;

const SIG: &[u8; 8] = b"EFI PART";

const PRIORITY_SHIFT: u64 = 48;
const ACTIVE_BIT: u64 = 1 << 50;
const RETRY_SHIFT: u64 = 51;
const SUCCESSFUL_BIT: u64 = 1 << 54;
const UNBOOTABLE_BIT: u64 = 1 << 55;
const AB_MASK: u64 = 0xFF << 48; // bits 48–55

/// CRC-32/ISO-HDLC (zlib), as GPT requires.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn rd_u64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

fn pread(f: &mut File, off: u64, len: usize) -> io::Result<Vec<u8>> {
    f.seek(SeekFrom::Start(off))?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf)?;
    Ok(buf)
}

/// Write `data` at `off`, honouring the device logical block size by
/// read-modify-writing the aligned superset (block devices reject sub-block
/// writes at odd offsets/lengths).
fn pwrite_aligned(f: &mut File, bs: u64, off: u64, data: &[u8]) -> io::Result<()> {
    let start = off - (off % bs);
    let end = {
        let e = off + data.len() as u64;
        e.div_ceil(bs) * bs
    };
    let mut region = pread(f, start, (end - start) as usize)?;
    let at = (off - start) as usize;
    region[at..at + data.len()].copy_from_slice(data);
    f.seek(SeekFrom::Start(start))?;
    f.write_all(&region)?;
    f.flush()
}

/// A parsed GPT header plus its raw entry array.
struct Gpt {
    hdr: Vec<u8>,       // header_size bytes
    hdr_off: u64,       // byte offset of this header on disk
    entries: Vec<u8>,   // num * size bytes
    entries_off: u64,   // byte offset of the entry array on disk
    num: u32,
    size: u32,
}

impl Gpt {
    fn read(f: &mut File, bs: u64, header_lba: u64) -> io::Result<Gpt> {
        let hdr_off = bs * header_lba;
        let raw = pread(f, hdr_off, 512.max(bs as usize).min(4096))?;
        if &raw[0..8] != SIG {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "no EFI PART signature"));
        }
        let header_size = rd_u32(&raw, 12) as usize;
        let hdr = raw[..header_size].to_vec();
        let entries_lba = rd_u64(&hdr, 72);
        let num = rd_u32(&hdr, 80);
        let size = rd_u32(&hdr, 84);
        let entries_off = bs * entries_lba;
        let entries = pread(f, entries_off, (num * size) as usize)?;
        Ok(Gpt { hdr, hdr_off, entries, entries_off, num, size })
    }

    fn entry_name(&self, i: u32) -> String {
        let base = (i * self.size) as usize + 56;
        let mut s = String::new();
        let mut o = base;
        while o + 1 < base + 72 {
            let u = u16::from_le_bytes([self.entries[o], self.entries[o + 1]]);
            if u == 0 {
                break;
            }
            s.push(char::from_u32(u as u32).unwrap_or('?'));
            o += 2;
        }
        s
    }

    fn entry_attrs(&self, i: u32) -> u64 {
        rd_u64(&self.entries, (i * self.size) as usize + 48)
    }

    fn set_entry_attrs(&mut self, i: u32, attrs: u64) {
        let o = (i * self.size) as usize + 48;
        self.entries[o..o + 8].copy_from_slice(&attrs.to_le_bytes());
    }

    fn find(&self, name: &str) -> Option<u32> {
        (0..self.num).find(|&i| self.entry_name(i) == name)
    }

    /// Recompute the entry-array CRC into the header, then the header CRC.
    fn refresh_crcs(&mut self) {
        let ecrc = crc32(&self.entries);
        self.hdr[88..92].copy_from_slice(&ecrc.to_le_bytes());
        self.hdr[16..20].copy_from_slice(&0u32.to_le_bytes());
        let hcrc = crc32(&self.hdr);
        self.hdr[16..20].copy_from_slice(&hcrc.to_le_bytes());
    }

    fn write_back(&self, f: &mut File, bs: u64) -> io::Result<()> {
        pwrite_aligned(f, bs, self.entries_off, &self.entries)?;
        pwrite_aligned(f, bs, self.hdr_off, &self.hdr)
    }
}

fn decode_ab(a: u64) -> String {
    format!(
        "priority={} active={} retry={} successful={} unbootable={}",
        (a >> PRIORITY_SHIFT) & 0x3,
        (a & ACTIVE_BIT != 0) as u8,
        (a >> RETRY_SHIFT) & 0x7,
        (a & SUCCESSFUL_BIT != 0) as u8,
        (a & UNBOOTABLE_BIT != 0) as u8,
    )
}

/// Attributes for a "known-good, preferred" slot: priority 3, active, 7 retries,
/// successful, not unbootable — preserving all non-A/B bits.
fn mark_good(a: u64) -> u64 {
    let ab = (3u64 << PRIORITY_SHIFT) | ACTIVE_BIT | (7u64 << RETRY_SHIFT) | SUCCESSFUL_BIT;
    (a & !AB_MASK) | ab
}

fn sibling_of(slot: &str) -> Option<&'static str> {
    match slot {
        "boot_a" => Some("boot_b"),
        "boot_b" => Some("boot_a"),
        _ => None,
    }
}

/// Make `slot` the chosen, sticky slot in this GPT: mark it good, and clear the
/// sibling's active bit so the bootloader unambiguously picks `slot` (the
/// sibling stays bootable as a fallback). Recomputes both CRCs. Returns false if
/// the target slot is absent.
fn apply_slot_choice(g: &mut Gpt, slot: &str) -> bool {
    let Some(i) = g.find(slot) else { return false };
    let a = g.entry_attrs(i);
    g.set_entry_attrs(i, mark_good(a));
    if let Some(sib) = sibling_of(slot) {
        if let Some(si) = g.find(sib) {
            let sa = g.entry_attrs(si);
            g.set_entry_attrs(si, sa & !ACTIVE_BIT);
        }
    }
    g.refresh_crcs();
    true
}

fn detect_bs(f: &mut File) -> io::Result<u64> {
    for &bs in &[4096u64, 512] {
        if let Ok(b) = pread(f, bs, 8) {
            if b == SIG[..] {
                return Ok(bs);
            }
        }
    }
    Err(io::Error::new(io::ErrorKind::InvalidData, "no primary GPT at LBA1 (tried 4096/512 block sizes)"))
}

fn slot_from_cmdline() -> Option<String> {
    let cl = std::fs::read_to_string("/proc/cmdline").ok()?;
    for tok in cl.split_whitespace() {
        if let Some(suf) = tok.strip_prefix("androidboot.slot_suffix=") {
            return Some(format!("boot{suf}"));
        }
    }
    None
}

fn print_gpt(tag: &str, f: &mut File, bs: u64, header_lba: u64) -> io::Result<()> {
    let g = Gpt::read(f, bs, header_lba)?;
    let stored_hcrc = rd_u32(&g.hdr, 16);
    let stored_ecrc = rd_u32(&g.hdr, 88);
    let mut h = g.hdr.clone();
    h[16..20].copy_from_slice(&0u32.to_le_bytes());
    let calc_hcrc = crc32(&h);
    let calc_ecrc = crc32(&g.entries);
    println!(
        "[{tag}] my_lba={} alt_lba={} entries_lba={} num={} size={}",
        rd_u64(&g.hdr, 24), rd_u64(&g.hdr, 32), rd_u64(&g.hdr, 72), g.num, g.size
    );
    println!(
        "[{tag}] header_crc: stored={:#010x} calc={:#010x} {}",
        stored_hcrc, calc_hcrc, if stored_hcrc == calc_hcrc { "OK" } else { "MISMATCH" }
    );
    println!(
        "[{tag}] entries_crc: stored={:#010x} calc={:#010x} {}",
        stored_ecrc, calc_ecrc, if stored_ecrc == calc_ecrc { "OK" } else { "MISMATCH" }
    );
    for name in ["boot_a", "boot_b"] {
        if let Some(i) = g.find(name) {
            let a = g.entry_attrs(i);
            println!("[{tag}] {name} (entry {i}) attrs={a:#018x}  {}", decode_ab(a));
        } else {
            println!("[{tag}] {name}: NOT FOUND");
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("dump");
    let commit = args.iter().any(|a| a == "--commit");
    let positional: Vec<&String> = args[1..].iter().filter(|a| !a.starts_with("--")).collect();
    let disk = positional.get(1).map(|s| s.as_str()).unwrap_or("/dev/sda");

    match cmd {
        "dump" => {
            let mut f = match File::open(disk) {
                Ok(f) => f,
                Err(e) => { eprintln!("bootctrl: open {disk}: {e}"); return ExitCode::FAILURE; }
            };
            let bs = match detect_bs(&mut f) {
                Ok(b) => b,
                Err(e) => { eprintln!("bootctrl: {e}"); return ExitCode::FAILURE; }
            };
            println!("[bootctrl] disk={disk} logical-block-size={bs}");
            if let Err(e) = print_gpt("primary", &mut f, bs, 1) {
                eprintln!("bootctrl: primary GPT: {e}"); return ExitCode::FAILURE;
            }
            // backup header LBA comes from the primary's alt_lba field.
            if let Ok(g) = Gpt::read(&mut f, bs, 1) {
                let alt = rd_u64(&g.hdr, 32);
                if let Err(e) = print_gpt("backup", &mut f, bs, alt) {
                    eprintln!("bootctrl: backup GPT ({alt}): {e}");
                }
            }
            ExitCode::SUCCESS
        }
        "mark" => {
            let slot_arg = positional.get(2).map(|s| s.as_str()).unwrap_or("auto");
            let slot = if slot_arg == "auto" {
                match slot_from_cmdline() {
                    Some(s) => s,
                    None => { eprintln!("bootctrl: no androidboot.slot_suffix in /proc/cmdline; pass a slot"); return ExitCode::FAILURE; }
                }
            } else {
                slot_arg.to_string()
            };
            let mut f = match OpenOptions::new().read(true).write(commit).open(disk) {
                Ok(f) => f,
                Err(e) => { eprintln!("bootctrl: open {disk}: {e}"); return ExitCode::FAILURE; }
            };
            let bs = match detect_bs(&mut f) {
                Ok(b) => b,
                Err(e) => { eprintln!("bootctrl: {e}"); return ExitCode::FAILURE; }
            };
            let mut primary = match Gpt::read(&mut f, bs, 1) {
                Ok(g) => g,
                Err(e) => { eprintln!("bootctrl: primary GPT: {e}"); return ExitCode::FAILURE; }
            };
            let alt = rd_u64(&primary.hdr, 32);
            let i = match primary.find(&slot) {
                Some(i) => i,
                None => { eprintln!("bootctrl: slot {slot} not found in GPT"); return ExitCode::FAILURE; }
            };
            let before = primary.entry_attrs(i);
            let after = mark_good(before);
            let sib = sibling_of(&slot);
            println!("[bootctrl] disk={disk} bs={bs} slot={slot} (entry {i})");
            println!("[bootctrl] {slot} before: attrs={before:#018x}  {}", decode_ab(before));
            println!("[bootctrl] {slot} after : attrs={after:#018x}  {}", decode_ab(after));
            if let Some(s) = sib {
                if let Some(si) = primary.find(s) {
                    let sb = primary.entry_attrs(si);
                    println!("[bootctrl] {s} before: attrs={sb:#018x}  {}", decode_ab(sb));
                    println!("[bootctrl] {s} after : active->0 (kept bootable as fallback)");
                }
            }
            if !commit {
                println!("[bootctrl] dry run (no --commit); wrote nothing");
                return ExitCode::SUCCESS;
            }
            // Apply the slot choice to primary and backup, then write both.
            apply_slot_choice(&mut primary, &slot);
            let mut backup = match Gpt::read(&mut f, bs, alt) {
                Ok(g) => g,
                Err(e) => { eprintln!("bootctrl: backup GPT ({alt}): {e}"); return ExitCode::FAILURE; }
            };
            let backup_has = apply_slot_choice(&mut backup, &slot);
            if !backup_has {
                eprintln!("bootctrl: WARNING slot {slot} not in backup GPT; writing primary only");
            }
            if let Err(e) = primary.write_back(&mut f, bs) {
                eprintln!("bootctrl: write primary: {e}"); return ExitCode::FAILURE;
            }
            if backup_has {
                if let Err(e) = backup.write_back(&mut f, bs) {
                    eprintln!("bootctrl: write backup: {e}"); return ExitCode::FAILURE;
                }
            }
            let _ = f.sync_all();
            println!("[bootctrl] committed; {slot} marked successful+active, sibling deactivated");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("bootctrl: unknown command '{other}' (use: dump | mark)");
            ExitCode::FAILURE
        }
    }
}
