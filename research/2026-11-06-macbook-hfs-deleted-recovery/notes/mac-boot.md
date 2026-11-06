# Booting Linux on a 2013–2014 Intel MacBook

## What the machine is

The relevant models are `MacBookPro11,1` (13-inch Retina, late 2013 / mid 2014),
`MacBookPro11,2` and `MacBookPro11,3` (15-inch Retina, late 2013 / mid 2014), and
`MacBookAir6,1` / `MacBookAir6,2` (11- and 13-inch, mid 2013 / early 2014). They are
64-bit Intel machines with 64-bit EFI firmware. They have **no T2 chip** and **no UEFI
Secure Boot** in the PC sense, so there is no “Startup Security Utility” and no need to
sign the bootloader. [s30]

## Startup keys

Apple’s list for Intel Macs: [s22]

- **Option (⌥) / Alt** — Startup Manager, choose another startup disk.
- **T** — Target Disk Mode.
- **Command-R** — built-in macOS Recovery; **Option-Command-R** / **Shift-Option-Command-R**
  for internet Recovery.
- **Command-Option-P-R** — reset NVRAM/PRAM.
- **D** — Apple Diagnostics; **Command-S** — single-user mode; **Command-V** — verbose.

A **firmware password** changes this: the firmware prompts for it before letting you use
Option, T, or another startup disk. If a lock icon appears, the password must be known or
the machine taken to Apple; there is no supported way around it. [s22]

## The USB must be UEFI-bootable

The Mac firmware will not boot a BIOS/legacy Linux USB. It looks for a FAT32 EFI System
Partition containing an EFI loader. The portable path is `EFI/BOOT/BOOTX64.EFI`; many
Linux ISOs provide this, and the Mac shows the device as **“EFI Boot”** in Startup
Manager. [s24]

Practical creation:

- On macOS, unmount the raw disk and write the ISO in **dd mode**:
  `sudo diskutil unmountDisk /dev/diskN` then
  `sudo dd if=image.iso of=/dev/rdiskN bs=1m`. Writing the file into a FAT32 partition is
  not the same as making a bootable hybrid image.
- On Linux, `dd if=image.iso of=/dev/sdX bs=4M status=progress oflag=sync`.
- Prefer a **GPT** partition table and a FAT32 ESP. Some ISOs are hybrid MBR and will
  still appear, but GPT/FAT32 is the least surprising.
- Try a **USB 2.0** stick and both ports if the Mac ignores a USB 3.0 stick. Early 2013
  machines have a known xHCI init bug that can make USB unusable until an SMC reset. [s27]

## If the Mac does not show the USB: rEFInd

rEFInd is the reliable fallback. Install it to a FAT32 USB (or to the Mac’s ESP) and let
it chainload the Linux EFI loader or the kernel directly. On macOS the registration step
is `bless`:

```
sudo bless --mount /Volumes/ESP --setBoot \
  --file /Volumes/ESP/efi/refind/refind_x64.efi --shortform
```

rEFInd’s documentation notes that Apple’s firmware already has its own HFS+ driver, so
rEFInd does not need an HFS+ driver on a Mac; it does need a driver for the filesystem
that holds the Linux kernel if it is not FAT. [s24]

## Ventoy: a known Mac regression

Ventoy is convenient but has a specific failure on these Macs. Versions 1.1.14 through
1.1.18 hang as soon as the USB is selected in the Mac boot menu; 1.1.12 works. The cause
is the new shim used for Secure Boot support, which is not compatible with Apple’s older
EFI. Workarounds: disable “Secure Boot support” when creating the Ventoy drive, or use
1.1.12. [s25][s26]

## Model-specific hardware quirks

- **15-inch dual GPU (MacBookPro11,2/11,3).** The panel is wired through the Apple GMUX,
  not directly to one GPU. Nouveau can produce a black screen. Add `nomodeset` or
  `modprobe.blacklist=nouveau` at the bootloader, and use the Intel i915 path. rEFInd’s
  `spoof_osx_version` can also change how the firmware exposes the GMUX. [s28]
- **Wi-Fi (BCM4360, `14e4:43a0`).** The in-kernel `b43` driver does not support this
  802.11ac chip; `brcmsmac` does not either. It needs the proprietary `broadcom-wl`
  driver, which is not on most live ISOs because of licensing. Plan for **offline**
  recovery, or use a USB/Thunderbolt Ethernet adapter. [s29][s27]
- **Keyboard/trackpad.** The internal keyboard and trackpad are USB devices handled by
  `bcm5974`; they normally work. If they do not, use a USB keyboard/mouse. [s27]
- **Internal SSD.** 2013–2014 Apple PCIe SSDs are AHCI-based (Samsung XP941/SM951
  family), not NVMe; Linux’s `ahci` driver handles them. 2015+ models are NVMe. If the
  disk is not detected, check `lspci -nn | grep -i sata` and `dmesg`. [s51]
- **SMC/xHCI.** If USB or the internal keyboard dies after boot, an SMC reset
  (Shift-Control-Option + power, 10 s, with the charger disconnected) is the documented
  workaround. [s27]

## Target Disk Mode

Hold **T** at startup to put the Mac in Target Disk Mode. It appears as an external disk
over FireWire or Thunderbolt. This is useful if you have another Mac, but Linux support
for Apple’s Thunderbolt Target Disk Mode is not reliable; FireWire SBP-2 is the mature
path and these models have no FireWire port without an adapter. [s23]

## The safest path: remove the SSD

Because these MacBooks use a removable proprietary PCIe SSD blade, the cleanest approach
is to take the SSD out and image it from another Linux machine. OWC’s Envoy Pro USB 3
enclosure explicitly lists `MacBookPro11,1`, `MacBookPro11,2`, `MacBookPro11,3`,
`MacBookAir6,1` and `MacBookAir6,2` as compatible. [s30]

Advantages: no firmware password, no GPU/Wi-Fi/USB quirks, no risk of macOS mounting and
writing to the volume, and a hardware write-blocker can be used. Caveat: verify the exact
SSD module; OWC’s list covers the factory Apple drives, not every aftermarket or repaired
module. [s30]

## What not to do

- Do not boot the installed macOS normally and start deleting/installing things; Spotlight,
  `mds`, `fseventsd` and ordinary writes can reuse the blocks you are trying to recover.
- Do not “repair” the original volume in place. Work on an image, or on a copy, and keep
  the original read-only.
- Do not run PhotoRec directly against a failing drive if it is making noises; image it
  with ddrescue/OpenSuperClone first. [s52][s46]
