# Booting Linux on a 2013–2014 MacBook and recovering deleted HFS+ files

*Research snapshot: 6 November 2026. This is a technical synthesis of Apple’s HFS+/APFS
documentation, upstream tool documentation, source code, and hardware reports — not a
laboratory recovery test on a specific MacBook. Citations use the `sNN` identifiers in
`sources.jsonl`; the same identifiers appear in `notes/`.*

## Bottom line

1. **Check the filesystem before choosing a tool.** The 2013–2014 Retina/Air models
   shipped with an SSD, and installing macOS High Sierra (10.13) or later on an internal
   SSD automatically converts the boot volume to APFS — “You can’t opt out” [s31]. HFS+
   survives on a machine that never went past Sierra, an external HDD, an external volume
   formatted HFS+, or a manually reformatted volume. If the volume is APFS, the HFS+ tools
   in the LinuxLinks list are the wrong branch; use APFS snapshots first [s33][s34].
2. **Check TRIM before spending hours.** Apple enables TRIM by default on Apple-supplied
   internal SSDs. After deletion the controller may unmap the blocks and return zeros, so
   the practical recovery window on the internal SSD is very small — often zero [s35][s36].
   An external USB hard drive is a much better candidate because TRIM usually does not
   pass through USB [s36].
3. **For a genuine HFS+ metadata deep scan, the only turnkey open-source tool found is
   `hfsprescue`.** It scans every allocation block of the partition for HFS+ catalog
   records, recovers deleted file/folder names, parent IDs, CNIDs, sizes, timestamps and
   extents, and supports HFS+ compression [s06][s07][s09]. It is GPL-2.0 and runs on Linux,
   macOS and FreeBSD [s08].
4. **PhotoRec is the best fallback, but it is a carver.** It ignores the filesystem and
   finds files by signature, so it recovers content without names, and it loses fragmented
   files, resource forks and `com.apple.decmpfs`-compressed files [s04][s10].
5. **TestDisk cannot undelete HFS+.** Its undelete feature is explicitly limited to FAT,
   exFAT, NTFS and ext2; its HFS+ support is partition/boot-sector repair [s02][s03].
6. **The Sleuth Kit cannot enumerate deleted HFS+ names either.** It reads the live
   catalog B-tree, marks every entry it reaches as allocated, and its HFS journal functions
   are stubs [s11][s12][s13]. `icat` still works if you already know a CNID; `blkls` can
   dump unallocated space for carving [s16][s17].
7. **ddrescue, safecopy, OpenSuperClone and DDRescue-GUI are imaging tools**, not scanners.
   Image first, then run a scanner on the image [s52][s46]. Mondo Rescue and
   Relax-and-Recover are backup/disaster-recovery tools, not undelete tools [s47][s48].
8. **Booting the Mac:** hold Option at the chime, pick “EFI Boot” from a GPT/FAT32 UEFI
   USB. A firmware password blocks this. If the USB is not shown, use rEFInd on a FAT32
   stick or `bless` a rEFInd installation [s22][s24]. Ventoy 1.1.14+ has a Mac-specific
   regression; disable Secure Boot support or use 1.1.12 [s25][s26].
9. **The safest physical path is to remove the Apple PCIe SSD** and image it in an OWC
   Envoy Pro (or equivalent) from another Linux machine. OWC lists the 2013–2014
   MacBookPro11,x and MacBookAir6,x drives as compatible [s30]. This avoids firmware,
   GPU, Wi-Fi and USB quirks and prevents macOS from writing to the volume.
10. **Expect names only from `hfsprescue`; expect content only from PhotoRec/Foremost/
    Scalpel/RecoverPy.** No open-source Linux tool in this list combines HFS+ metadata
    recovery with robust handling of every fragmented/compressed file. Plan for a
    best-effort result, not a complete restore.

## 1. The assumption to check first: HFS+, APFS, or FileVault

### 1.1 APFS conversion

Apple’s rule is unambiguous: installing macOS High Sierra on an SSD or other all-flash
storage device automatically converts the volume to APFS; Fusion Drives, traditional HDDs
and non-Mac volumes are not converted; the user cannot opt out [s31]. The conversion is a
nondestructive in-place upgrade, and reverting to HFS+ requires erasing the volume and
loses the data [s32].

A 2013–2014 MacBook (MacBookPro11,1/11,2/11,3, MacBookAir6,1/6,2) shipped with an SSD
[s30]. (The older non-Retina 13-inch MacBook Pro, Mid 2012, was still sold in this window
and could have a hard drive; a hard-drive machine would stay HFS+.) If the machine ever
ran High Sierra or later on that internal SSD, it is almost certainly APFS. A 2013–2014
machine that stayed on Sierra or earlier, or whose data lives on an external HFS+ hard
drive, is the HFS+ case this report focuses on.

### 1.2 Read-only identification

On macOS:

```
diskutil list
diskutil info disk0s2        # "Type (Bundle)", "File System Personality", "Encrypted"
diskutil apfs list           # APFS containers, volumes, snapshots
diskutil info disk0s2 | grep -i -E 'Type|File System|Encrypted|TRIM'
```

For an APFS volume, use the synthesised volume identifier from `diskutil list` (usually
`disk3s1`, not `disk0s2`) with `diskutil info` and `diskutil apfs listSnapshots`.

On Linux, against the image or device:

```
lsblk -f
blkid
file -s /dev/sdX2
mmls /dev/sdX                       # partition layout and sector offset
fsstat -o <sector-offset> /dev/sdX  # reports hfs or apfs
```

TSK’s `fsstat` autodetects HFS+ and APFS (APFS support since TSK 4.8) [s34]. If it says
`apfs`, stop and use the APFS branch in §7.3. If it says `hfs`, continue.

### 1.3 FileVault 2 / CoreStorage

On Lion through Sierra, FileVault 2 wraps the HFS+ volume in a CoreStorage logical volume;
the raw partition is “Apple CoreStorage”, not HFS+. `libfvde` can unlock it with the user
password, recovery key, or `EncryptedRoot.plist.wipekey` [s44]:

```
fvdeinfo disk.img
fvdemount -o <partition-offset-bytes> -p '<password>' disk.img /mnt/fvde
# or -r <recovery-key>, or -e EncryptedRoot.plist.wipekey
```

Then scan `/mnt/fvde/fvde1`. `libfvde` is experimental, supports Lion through Catalina
CoreStorage, and does not support multiple physical volumes or volumes ≥1 TiB [s44]. On
APFS, encryption is native APFS; `apfs-fuse` can mount encrypted volumes with
`-r <password-or-PRK>` [s33].

### 1.4 TRIM

TRIM is the quiet killer. macOS enables it for internal Apple SSDs [s35]. When a file is
deleted, the SSD is told the blocks are no longer needed; the controller may drop the
mapping and return deterministic zeros (RZAT) on later reads [s36]. The exact timing
depends on the controller, firmware and garbage collection, but the practical recovery
window on an internal Apple SSD is very small — often zero [s36]. External USB drives
usually do not pass TRIM, so deleted data there remains recoverable [s36]. A third-party
internal SSD with `trimforce` never enabled may also retain data [s35].

This does not make recovery impossible in every case; it makes it a gamble. If the data
is on the internal SSD and the machine has been powered on and used since the deletion,
set expectations accordingly.

## 2. What “deleted” means on HFS+

### 2.1 The structures that matter

- **Volume header** at byte offset 1024; an alternate copy 1024 bytes before the end. It
  records the allocation block size and the fork descriptors for the catalog, extents,
  attributes and allocation files [s18].
- **Catalog B-tree** (`$Catalog`): the directory. Leaf records are folder (type 1), file
  (2), folder thread (3) and file thread (4). The key is parent ID + Unicode name; the data
  holds the CNID, timestamps, permissions and the first eight extents of each fork [s18].
- **Extents overflow B-tree** (`$ExtentsFile`): additional extents keyed by
  fileID/forkType/startBlock [s18].
- **Attributes B-tree** (`$AttributesFile`): extended attributes, including
  `com.apple.decmpfs` [s18][s10].
- **Allocation file** (`$AllocationFile`): the free/allocated block bitmap [s18].
- **Journal**: a transactional record used to restore consistency after an unclean
  unmount [s18].

### 2.2 What deletion removes

macOS removes the file’s catalog record from the B-tree and marks the file’s blocks free
in the allocation bitmap [s19][s20]. The data blocks are not erased at that moment; they
remain until reallocated and overwritten [s19]. Extent records in the extents overflow
B-tree can remain keyed by the old CNID, and the journal can retain the catalog transaction
for a while [s18][s19][s21].

Crucially, HFS+ does **not** leave a directory entry marked “deleted” the way FAT does.
The record is removed. That is why `fls -d` has nothing to list on a healthy HFS+ volume,
and why TestDisk’s undelete code — which depends on a deleted directory entry — does not
apply [s02][s03][s12].

### 2.3 Where recoverable evidence survives

| Layer | What it finds | Names/paths? | Tool |
|---|---|---|---|
| Stale catalog records in unused B-tree nodes | Deleted file/folder records that were not overwritten | Yes, if the record survives | `hfsprescue` [s09] |
| Extents overflow records | Block runs for a CNID whose catalog record is gone | CNID only; name may be missing | `hfsprescue --find-eof`, manual parsing [s07] |
| Journal | Recent catalog transactions with attributes and extents | Sometimes | Research only; TSK stubs [s21][s13] |
| Free data blocks | File content by signature | No | PhotoRec, Foremost, Scalpel, RecoverPy [s04][s43] |
| Live catalog | Files that still exist | Yes | TSK `fls`, `hfsplus` [s10][s50] |

A “deep scan” that only reads the live catalog is not a deleted-file scan. A “deep scan”
that only carves is a data-level scan with no names. `hfsprescue` is the only open-source
tool found that scans for the *deleted metadata* layer on HFS+.

### 2.4 Compression, resource forks and hard links

- **HFS+ compression**: compressed files have an empty data fork. The content is in
  `com.apple.decmpfs` and/or the resource fork, compressed with zlib, LZVN or LZFSE. A
  carver that only reads the data fork sees nothing [s10]. TSK 4.0+ understands
  `com.apple.decmpfs` and decompresses on `icat`; `hfsprescue` advertises compression
  support; PhotoRec does not [s10][s06][s04].
- **Resource forks**: a second fork; most user files have none, but some applications use
  it. TSK exposes it as attribute `4353-1`; a data-fork-only carver loses it [s10].
- **Hard links**: files in `/^^^^HFS+ Private Data` named `iNode<cnid>`; directory hard
  links used by Time Machine live in `/HFS+ Private Directory Data<CR>` as `dir_<n>` [s10].
  Recovery tools that ignore these produce missing or duplicated content.

### 2.5 The journal is not a turnkey path

The DFRWS work by Burghardt and Carrier showed that HFS+ journal transactions can retain
file attributes and extents for recent deletions, and that journal-based recovery is more
accurate than free-block carving [s21]. But no maintained open-source implementation was
found. TSK’s `hfs_journal.cpp` is explicit:

```
jopen not implemented for HFS yet
jentry_walk not implemented for HFS yet
jblk_walk not implemented for HFS yet
```

[s13]

So the journal is a research avenue and a reason to image *immediately* after a deletion
(the journal wraps quickly), not a button you can press in a current Linux tool.

## 3. The direct answer: which tools can deep-scan HFS+

### 3.1 `hfsprescue` — the one genuine HFS+ metadata deep scanner

`hfsprescue` is a GPL-2.0 program by Elmar Hanlhofer that recovers files from HFS+
partitions that cannot be mounted. Its own documentation says the quiet part out loud:
“As side effect, the program also restores deleted files” [s06]. It runs on Linux, macOS
and FreeBSD, supports HFS+ compression, and the latest release is 3.6 (07/Apr/2023) [s07].

**How it works.** The source is the proof. Step 1 does not walk the live catalog B-tree.
It reads the partition block by block, and `ScanBuffer()` slides through each block
byte-by-byte looking for a plausible catalog key:

```
key_len == filename_len * 2 + 6
filename_len > 0 && filename_len <= 256
type > 0 && type < 5        // folder, file, folder thread, file thread
```

[s09]

When it finds one, it reads the parent ID, the Unicode name, the record type, the CNID,
the timestamps, the file size, the compressed flag, and the first eight extents [s09].
That is a raw **metadata** scan: it can find records in blocks that the live B-tree no
longer references, which is exactly what “deleted files not listed in a directory” means.

**Why that matters.** A carver can recover a JPEG’s bytes but not its name, creation date
or folder. `hfsprescue` can recover the name, parent folder, CNID and extent list — if the
catalog record bytes survived. That is the difference between a usable recovery and a
`recup_dir.1/file000123.jpg` pile.

**Limits:**

- It is slow: every block is scanned, so a large or failing volume takes a long time. Image
  first and run it on the image [s09][s07].
- It can produce false positives; step 2 (`-s2`) cleans the file database, and the manual
  warns that older/deleted versions may be corrupted [s07].
- It preserves what the record says. If the record was overwritten, the file is gone from
  the metadata path.
- It is packaged in Kaisen Linux [s40] and the AUR, and builds from source; it is not part
  of the standard Debian/Ubuntu package set as far as this search could establish.
- Last release 2023; no independent benchmark of its HFS+ deleted-file accuracy was found.

### 3.2 PhotoRec — a data-level deep scan, no names

PhotoRec “ignores the file system and goes after the underlying data”, recovering files by
signature [s04]. It supports HFS+ in the sense that it can read an HFS+ partition’s blocks
and carve known file types; it does not read HFS+ metadata, so it cannot recover names or
paths [s04].

What that means in practice:

- Good for contiguous JPEG, PDF, ZIP, DOCX, MP4 and similar files whose headers/footers
  are recognisable.
- Bad for fragmented files: PhotoRec expects non-fragmented data, and the TestDisk manual
  says so explicitly [s03].
- Bad for HFS+ resource forks and `com.apple.decmpfs`-compressed files: the data fork is
  empty [s10].
- The output is `recup_dir.N` with sequential names; sorting is manual.

PhotoRec’s menu asks for the filesystem type after you select a partition. For HFS+ choose
**Other** (not ext2/3/4); then choose **Whole** if **Free** is not offered. PhotoRec’s
free-space mode is documented mainly for FAT/NTFS and may not be offered for HFS+ [s05].

### 3.3 TestDisk — not an HFS+ undelete tool

TestDisk’s own feature list is explicit: undelete from FAT, exFAT, NTFS and ext2; HFS+
support is “restore the boot sector using its backup” [s02]. The 7.2 manual repeats the
same list [s03]. TestDisk can recover a lost HFS+ *partition* and repair its boot sector,
which is useful if the volume no longer appears at all. It cannot recover a deleted file
whose catalog record is gone.

This is the most common confusion in the roundup: a tool that “supports HFS+” for
partition repair is not a tool that undeletes HFS+ files.

### 3.4 The Sleuth Kit — read-only HFS+ analysis, no deleted enumeration

TSK supports HFS+ and HFSX, and since 4.0 it supports resource forks, extended attributes,
hard links, symlinks and compressed files [s10]. It is excellent for reading the live
filesystem, extracting a known file by CNID, and dumping unallocated space.

It is **not** an HFS+ undelete tool:

- `hfs_cat_traverse()` starts at the catalog B-tree root and follows `flink` through live
  leaf nodes; deleted records are no longer in that chain [s11].
- `hfs_dir_open_meta_cb()` sets `TSK_FS_NAME_FLAG_ALLOC` for every folder and file entry;
  there is no deleted branch [s12].
- The journal functions are stubs [s13].
- The 2010 OSDFCon deck said the same thing: for HFS+, “Deleted files” and “Journal
  information” did not work [s14]. The source still reflects that limitation.

So `fls -d` on HFS+ is generally empty. `tsk_recover`’s default unallocated-file mode
depends on the filesystem’s ability to enumerate unallocated metadata, which TSK’s HFS+
implementation does not provide; `tsk_recover -a` / `-e` can still export live files
[s15][s16]. What *does* work:

```
fls -f hfs -o <sector> -r -p image.img          # live files
istat -f hfs -o <sector> image.img <cnid>       # metadata for a known CNID
icat  -f hfs -o <sector> image.img <cnid> > f   # content for a known CNID
blkls -f hfs -o <sector> -A image.img > free.img # unallocated blocks for carving
```

[s16][s17]

`icat` is valuable when `hfsprescue` or a manual catalog scan gives you a CNID. `blkls`
is the bridge to PhotoRec/Foremost/Scalpel.

### 3.5 The rest of the roundup, in one line each

| Tool | Verdict |
|---|---|
| **ddrescue** | Imaging. Use it first; it does not parse HFS+ [s52]. |
| **SystemRescue** | Live distro with TSK 4.15, TestDisk 7.2, ddrescue, Foremost. No `hfsprescue` [s37]. |
| **Relax-and-Recover** | Bare-metal disaster recovery, not file undelete [s47]. |
| **RecoverPy** | Raw byte-pattern search; finds text/content fragments, not filenames or file boundaries [s43]. |
| **Mondo Rescue** | Backup/DR, not undelete [s48]. |
| **Rescuezilla** | Live distro with TestDisk + PhotoRec; its “deep scan” is PhotoRec [s39]. |
| **OpenSuperClone** | Sector-by-sector imaging of failing media [s46]. |
| **Mariposa Rescue Disk** | Debian live with sleuthkit and a broad rescue toolset; verify the current ISO’s package list [s41]. |
| **safecopy** | Imaging that ignores read errors; no filesystem parser [s01]. |
| **Redo Rescue** | Live distro / backup in the Redo Backup lineage; verify the current ISO’s recovery tools [s01][s39]. |
| **Foremost** | Signature carver; no HFS+ metadata [s01]. |
| **DDRescue-GUI** | GUI for ddrescue; no scanner [s01]. |
| **Scalpel** | Signature carver; no HFS+ metadata [s01]. |
| **ext4magic** | ext2/3/4 only; cannot read HFS+ [s01]. |
| **RevLix** | GUI wrapper around PhotoRec/TestDisk/ddrescue/Foremost; no new HFS+ engine [s42]. |

### 3.6 Tools outside the roundup worth knowing

- **HFSleuth**: a free (not open-source) HFS+/HFSX inspection tool that runs on Linux and
  can traverse volumes/images; useful for manual catalog/B-tree inspection [s45]. Not a
  one-click undelete.
- **hfsplus / hfsprogs / hfsutils**: mount, list, copy, fsck. `fsck.hfsplus -r` rebuilds
  the catalog and can move orphans to `lost+found`; that is repair, not a deleted-file
  scan [s49][s50].
- **libfvde**: FileVault 2/CoreStorage unlock [s44].
- **apfs-fuse / TSK APFS**: the APFS branch [s33][s34].

## 4. `hfsprescue` in practice

### 4.1 Install

- **Kaisen Linux System Rescue ISO** already includes `hfsprescue` in its data-recovery
  set, along with `hfsplus`, `hfsprogs`, `libfvde-utils` and `apfs-fuse` [s40]. This is
  the most convenient live environment for an HFS+ job.
- On Debian/Ubuntu, build from source (the project uses autotools; `./configure && make &&
  sudo make install`) [s07][s08]. The author also offers precompiled binaries, but the
  download page marks them outdated.
- On Arch, the AUR package is `hfsprescue`.
- On macOS, the author’s macOS build can scan the disk directly, but running from the
  affected volume is risky; boot from another volume or attach the disk externally.

### 4.2 The recovery sequence

```
mkdir -p /mnt/evidence/hfsp && cd /mnt/evidence/hfsp

# 1. Scan every block for catalog records
hfsprescue -s1 /mnt/evidence/mac.img -o <partition-start-bytes>

# 2. Clean up the file database (charset, future dates)
hfsprescue -s2

# Inspect what was found before restoring
hfsprescue --list | less
hfsprescue --csv files.csv

# 3. Restore; --alternative keeps deleted/older versions with a new name
hfsprescue -s3 /mnt/evidence/mac.img -o <partition-start-bytes> --alternative

# 4. Rebuild the directory structure
hfsprescue -s4

# 5. Move restored files into the directories
hfsprescue -s5

# 6. Finalize/cleanup
hfsprescue -s6
```

[s07][s08]

- `-o` is the partition start in **bytes** (not sectors), useful when the partition table
  is damaged or you are scanning a whole-disk image.
- `-o` sets the start but not the end: on a whole-disk image the scan continues past the
  HFS+ partition. For a bounded, faster scan, image the partition itself
  (`ddrescue /dev/sdX2 partition.img mapfile`) or use a partition-only image.
- `-b` sets the allocation block size if the boot sector is lost (typically 4096).
- `--one-file <image> <number>` restores a single file from the `--list` numbering.
- If the volume header is damaged: `--find-vh`, `--find-avh`, `--find-eof`,
  `--extract-eof`, `--extract-vh` [s07].
- The scan is read-only: the manual states the HFS+ filesystem is opened read-only [s06].

### 4.3 What to expect

- Deleted files that were recently removed, whose catalog records still sit in unused
  B-tree nodes, can come back with names and folder structure.
- Files deleted long ago, or after heavy writes, will be missing or truncated.
- `--alternative` produces names like `originalname_<CNID>` when a live file already
  occupies the name; the manual warns deleted/older versions may be corrupted [s07].
- The output can contain false positives. Step 2 is not optional; review `--list` before
  restoring everything.

### 4.4 “Deep scan” vs commercial deep scan

Commercial tools (Disk Drill, R-Studio, UFS Explorer) advertise a “deep scan” that combines
filesystem-aware reconstruction with carving. `hfsprescue` is closer to the
filesystem-aware half: it scans for catalog records and rebuilds names/extents. PhotoRec is
the carving half. Using both, in that order, is the open-source approximation of a
commercial deep scan [s19][s20].

## 5. PhotoRec and the carving fallback

### 5.1 When to use it

Use PhotoRec when:

- `hfsprescue` finds nothing or the catalog is destroyed.
- You need content from a volume that will not mount.
- You are working on an image and want a second opinion.

Do not use it as the first tool on a healthy HFS+ volume: it will recover live files too,
producing duplicates, and it cannot give you names.

### 5.2 HFS+ specifics

- Choose the HFS+ partition, or “No partition” for a whole-disk scan.
- Choose **Other** for the filesystem type; HFS+ is not ext2/3/4 [s05].
- Choose **Whole** if **Free** is not offered. PhotoRec’s free-space mode is documented
  mainly for FAT/NTFS and may not be offered for HFS+; alternatively use `blkls` to
  extract unallocated blocks and run PhotoRec on that file [s05][s17].
- File Opt: disable all, enable only the types you need. This is the single biggest
  quality-of-life improvement.
- Fragmented files will be partial or broken; PhotoRec expects non-fragmented data [s03].
- Resource forks and compressed files are lost [s10].

### 5.3 Foremost, Scalpel, RecoverPy

- **Foremost** and **Scalpel** are configuration-driven signature carvers. They are
  filesystem-agnostic: they will find JPEG/PDF/ZIP etc. on an HFS+ image, but they know
  nothing about HFS+ names or extents [s01].
- **RecoverPy** scans the raw partition for a byte pattern and lets you inspect adjacent
  blocks. It is excellent for “I remember a phrase from the document” but it explicitly
  does not interpret filesystem structures or restore filenames [s43]. A plain
  `grep -abo --binary-files=text 'phrase' image.img` does something similar without the
  TUI.

## 6. Booting a Linux live USB on the Mac

### 6.1 The machine

The relevant models are `MacBookPro11,1` (13-inch Retina, late 2013 / mid 2014),
`MacBookPro11,2`/`11,3` (15-inch Retina, late 2013 / mid 2014), and `MacBookAir6,1`/`6,2`
(11/13-inch, mid 2013 / early 2014) [s30]. They are 64-bit Intel machines with 64-bit EFI.
They predate the T2 (introduced across Mac models in 2017–2018) and do not enforce UEFI
Secure Boot in the PC sense, so there is no “Startup Security Utility” and no need to sign
the bootloader.

### 6.2 Startup keys and the firmware password

Apple’s Intel-Mac list [s22]:

- **Option (⌥) / Alt** — Startup Manager.
- **T** — Target Disk Mode.
- **Command-R** — Recovery; **Option-Command-R** / **Shift-Option-Command-R** — internet
  Recovery.
- **Command-Option-P-R** — reset NVRAM/PRAM.
- **D** — Apple Diagnostics; **Command-S** — single-user; **Command-V** — verbose.

A **firmware password** prompts before these alternate startup choices and blocks selecting
another disk [s22]. If you see a lock, you need the password or Apple’s help; there is no
supported bypass.

### 6.3 Make the USB UEFI-bootable

The Mac will not boot a BIOS/legacy Linux USB. It needs a FAT32 EFI System Partition with
an EFI loader; the portable path is `EFI/BOOT/BOOTX64.EFI`. The Mac shows it as
**“EFI Boot”** in Startup Manager [s24].

- On macOS: `sudo diskutil unmountDisk /dev/diskN`, then
  `sudo dd if=image.iso of=/dev/rdiskN bs=1m`. Use the raw device (`rdiskN`) for speed.
- On Linux: `sudo dd if=image.iso of=/dev/sdX bs=4M status=progress oflag=sync`.
- Prefer GPT + FAT32. Some hybrid ISOs work, but GPT/FAT32 is the least surprising.
- Try a USB 2.0 stick and both ports. Early 2013 machines have an xHCI init bug that can
  make USB unusable until an SMC reset [s27].

### 6.4 If the USB is not shown: rEFInd

rEFInd is the reliable fallback. Put it on a FAT32 USB (or the Mac’s ESP) and let it
chainload the Linux EFI loader or the kernel directly. On macOS, register it with:

```
sudo bless --mount /Volumes/ESP --setBoot \
  --file /Volumes/ESP/efi/refind/refind_x64.efi --shortform
```

[s24]

Apple’s firmware already has an HFS+ driver, so rEFInd does not need one on a Mac; it
does need a driver for the filesystem holding the Linux kernel if that is not FAT [s24].
If macOS will not boot, run the same `bless` command from Recovery’s Terminal
(Utilities → Terminal).

### 6.5 Ventoy

Ventoy is convenient but has a specific Mac regression. Versions 1.1.14–1.1.18 hang when
the USB is selected in the Mac boot menu; 1.1.12 works. The cause is the new Rocky Linux
shim used for Secure Boot support, which is not compatible with Apple’s older EFI.
Workarounds: disable “Secure Boot support” when creating the Ventoy drive, or use
1.1.12 [s25][s26].

### 6.6 Model-specific quirks

- **15-inch dual GPU.** The panel is wired through the Apple GMUX. Nouveau can black-screen;
  add `nomodeset` or `modprobe.blacklist=nouveau`, and use Intel i915. rEFInd’s
  `spoof_osx_version` can change how the firmware exposes the GMUX [s28].
- **Wi-Fi.** The BCM4360 (`14e4:43a0`) is not supported by `b43` for 802.11ac; it needs
  the proprietary `broadcom-wl` driver, which is absent from most live ISOs [s29][s27].
  Plan offline, or use a USB/Thunderbolt Ethernet adapter.
- **Keyboard/trackpad.** `bcm5974` normally works; otherwise use USB input [s27].
- **Internal SSD.** 2013–2014 Apple PCIe SSDs are AHCI-based (Samsung XP941/SM951 family),
  handled by Linux’s `ahci` driver; 2015+ are NVMe [s51].
- **SMC/xHCI.** If USB or the internal keyboard dies after boot, an SMC reset (Shift-Control-
  Option + power, 10 s, charger disconnected) is the documented workaround [s27].

### 6.7 Target Disk Mode

Hold **T** at startup. The Mac appears as an external disk over FireWire or Thunderbolt
[s23]. This is useful if you have another Mac, but Linux support for Apple’s Thunderbolt
Target Disk Mode is not reliable; FireWire SBP-2 is the mature path, and these models need
an adapter for FireWire [s23].

### 6.8 The safest path: remove the SSD

These MacBooks use a removable proprietary PCIe SSD blade. OWC’s Envoy Pro USB 3 enclosure
lists `MacBookPro11,1`, `MacBookPro11,2`, `MacBookPro11,3`, `MacBookAir6,1` and
`MacBookAir6,2` as compatible [s30]. Remove the SSD, put it in the enclosure, attach it to
another Linux machine, and image it there.

Advantages: no firmware password, no GPU/Wi-Fi/USB quirks, no macOS writes, and a hardware
write-blocker can be used. Caveat: verify the exact SSD module; OWC’s list covers the
factory Apple drives, not every aftermarket or repaired module [s30].

### 6.9 Which live ISO to pick

- **Kaisen Linux System Rescue** is the best fit for an HFS+ job: it is Debian-based,
  includes `hfsprescue`, `hfsplus`, `hfsprogs`, `libfvde-utils` and `apfs-fuse`, and has a
  dedicated rescue ISO [s40]. It is the only live environment found that ships the one
  HFS+ metadata scanner out of the box.
- **SystemRescue** is the best general-purpose rescue environment: it includes TSK 4.15,
  TestDisk 7.2, PhotoRec, ddrescue and Foremost, but not `hfsprescue` [s37][s38]. You can
  build `hfsprescue` inside it if you have the toolchain and source, or use it for the
  TSK/PhotoRec half of the workflow.
- **Rescuezilla / Redo Rescue** are the easiest GUI options, but their “deep scan” is
  PhotoRec and their “file recovery” is TestDisk, so they inherit the HFS+ limits described
  above [s39].
- **Ubuntu / Fedora** have the best Mac hardware support (trackpad, keyboard, GPU, USB) and
  can install `testdisk`, `sleuthkit` and `photorec` from their repositories, but you will
  need to build `hfsprescue` yourself. On a 15-inch dual-GPU Mac, remember `nomodeset`.

Whichever you choose, verify the ISO boots to a graphical or text environment **before**
you need it, and keep a second USB stick with a different distro as a fallback.

### 6.10 What not to do

- Do not boot the installed macOS normally and start installing/deleting things; Spotlight,
  `mds`, `fseventsd` and ordinary writes can reuse the blocks.
- Do not “repair” the original volume in place. Work on an image or a copy.
- Do not run PhotoRec against a noisy failing drive; image it with ddrescue/OpenSuperClone
  first [s52][s46].

## 7. Recommended workflows

### 7.1 Decision tree

```
Can you still boot macOS?
├─ yes → do NOT mount/write the affected volume
│         image it from macOS with ddrescue (or remove the SSD)
└─ no  → boot Linux from USB, or remove the SSD and image elsewhere

Identify the volume:
├─ APFS → §7.3
├─ CoreStorage/FileVault → libfvde → then HFS+ path
└─ HFS+ → §7.2
```

### 7.2 HFS+ path (the main case)

1. **Stop using the machine.** Every write reduces the chance.
2. **Image** the whole disk with ddrescue; hash the image; work on a copy [s52].
3. **Identify** the partition offset with `mmls`; confirm `fsstat` says `hfs` [s10][s34].
4. **Try `hfsprescue` first.** `-s1`, `-s2`, `--list`, `-s3 --alternative`, `-s4`, `-s5`,
   `-s6` [s07]. This is the only open-source path found that preserves names and folders
   for deleted HFS+ files.
5. **TSK triage.** `fls -r` for live files; `icat` for known CNIDs; `blkls -A` to extract
   unallocated space [s16][s17].
6. **Carve.** PhotoRec on the image or on `blkls` output; Foremost/Scalpel if you prefer
   configuration files [s04][s05].
7. **Search text.** RecoverPy or `grep -abo` for a distinctive phrase [s43].
8. **Repair only on a copy.** `fsck.hfsplus -n` first; `-r` to rebuild the catalog; check
   `lost+found` [s49].
9. **If encrypted,** unlock with `libfvde` and scan `fvde1` [s44].

### 7.3 APFS path

1. **Snapshots first.** On macOS: `tmutil listlocalsnapshots /`,
   `diskutil apfs listSnapshots disk3s1`. On Linux from the image: `apfs-fuse` with
   `snap=<xid>` after listing with `apfsutil` [s33].
2. **Mount read-only.** `apfs-fuse -o ro,vol=1 image.img /mnt/apfs` [s33].
3. **TSK APFS** for offset-precise extraction; note limited encryption support [s34].
4. **Carve** with PhotoRec if the object map is gone. Expect no names.
5. **Do not** run `hfsprescue`, TestDisk, `hfsprogs` or `hfsplus` on APFS.

### 7.4 FileVault 2 path

1. Image the whole disk.
2. `fvdeinfo` to identify the CoreStorage volume.
3. `fvdemount` with password, recovery key, or `EncryptedRoot.plist.wipekey`.
4. Scan `/mnt/fvde/fvde1` with `hfsprescue`/TSK/PhotoRec [s44].

### 7.5 Failing drive

- Image with ddrescue (`-n` first pass, then `-r3`) or OpenSuperClone’s multi-phase clone
  [s52][s46].
- Do not run PhotoRec on a drive that is physically degrading; it will stress it.
- If ddrescue stalls or the drive makes unusual noises, the drive is likely dying; a
  professional imaging lab may be the only option.

## 8. Disagreements, uncertainties and what the sources do not say

1. **“TSK recovers deleted files” vs HFS+.** TSK’s general description says deleted and
   hidden content is shown [s53], and its NTFS/FAT/ext support is real. But for HFS+ the
   source marks every reached catalog entry allocated and the journal is stubbed
   [s11][s12][s13]; the 2010 OSDFCon deck said deleted files did not work for HFS+ [s14].
   The general marketing claim does not hold for HFS+ deleted names. This is the clearest
   conflict in the sources.
2. **“TestDisk supports HFS+” vs undelete.** TestDisk supports HFS+ for partition and boot
   sector repair, not for file undelete [s02][s03]. Users and some secondary articles
   conflate the two.
3. **“PhotoRec supports HFS+” vs names.** PhotoRec supports HFS+ as a source of blocks,
   not as a metadata source [s04]. It is a carver.
4. **`hfsprescue`’s deleted-file claim is a side effect, not a guarantee.** The author says
   it restores deleted files, and the source shows why it can, but the manual also warns
   that deleted/older versions may be corrupted [s06][s07]. No independent benchmark was
   found. Treat the output as evidence to review, not a complete restore.
5. **TRIM “unrecoverable” is a practical expectation, not a law.** Controller behaviour,
   firmware, garbage-collection timing and whether the drive was powered on all matter
   [s36]. Some SSDs delay TRIM; some USB bridges do not pass it. But on an internal Apple
   SSD, assume the window is small.
6. **The HFS+ assumption is the biggest risk.** A 2013–2014 MacBook that ran High Sierra+
   on its SSD is APFS [s31]. The report branches, but the user must check before running
   any HFS+ tool.
7. **Ventoy/Mac compatibility is version-specific.** The regression is documented for
   1.1.14–1.1.18 and for MacBookPro11,1; other versions/models may behave differently
   [s25][s26].
8. **2013–2014 SSD interface.** The “PCIe AHCI” description is well supported by the
   community [s51], but aftermarket NVMe upgrades and firmware versions can change what
   Linux sees. Check `lspci`/`dmesg` rather than assuming.
9. **`hfsprescue` packaging.** It is in Kaisen and the AUR, not in the standard
   Debian/Ubuntu package set as far as this search could establish [s40]. “Install it on
   Linux” may mean compile it or use Kaisen.
10. **HFS+ journal recovery** is a research method, not a maintained tool [s21][s13]. Do
    not expect a `--journal` flag in any current open-source recovery program.

## 9. What would improve the answer

- A controlled test: create an HFS+ image, write known files, delete them, run
  `hfsprescue`/TSK/PhotoRec, and measure name/path/content recovery by file type and
  fragmentation. No such open benchmark was found.
- A current `hfsprescue` accuracy report on macOS-created HFS+ volumes (HFSX, compression,
  resource forks, hard links).
- A maintained open-source HFS+ journal parser; the DFRWS method is 2008-era and not
  packaged.
- A 2026 re-test of Ventoy and rEFInd on MacBookPro11,1/11,3 with the latest ISOs.
- A TRIM study on 2013–2014 Apple SSDs measuring how quickly deleted blocks become
  unreadable; the current evidence is mechanism-level, not model-specific.

## Sources

The `sNN` identifiers below match `sources.jsonl`. Primary sources are marked ★.

- ★ [s01] LinuxLinks, “18 Best Free and Open Source Linux Data Recovery Tools,” 2026-07-04.
  https://www.linuxlinks.com/DataRecovery/
- ★ [s02] CGSecurity, “Partition Recovery and File Undelete.”
  https://www.cgsecurity.org/wiki/TestDisk
- ★ [s03] CGSecurity, “TestDisk Documentation, Release 7.2.”
  https://www.cgsecurity.org/testdisk.pdf
- ★ [s04] CGSecurity, “Digital Picture and File Recovery.”
  https://www.cgsecurity.org/wiki/PhotoRec
- ★ [s05] CGSecurity, “PhotoRec Step By Step.”
  https://www.cgsecurity.org/wiki/PhotoRec_Step_By_Step
- ★ [s06] Plop, “HFS+ Rescue – Introduction.”
  https://www.plop.at/en/hfsprescue/intro.html
- ★ [s07] Plop, “HFS+ Rescue – full reference.”
  https://www.plop.at/en/hfsprescue/full.html
- ★ [s08] Kaisen Linux, “hfsprescue” repository and man page.
  https://github.com/kaisenlinux/hfsprescue
- ★ [s09] hfsprescue source, `src/hfsprescue.cpp` (ScanBuffer / scan loop).
  https://github.com/kaisenlinux/hfsprescue/blob/master/src/hfsprescue.cpp
- ★ [s10] The Sleuth Kit Wiki, “HFS.”
  https://wiki.sleuthkit.org/HFS
- ★ [s11] The Sleuth Kit source, `tsk/fs/hfs.cpp`.
  https://github.com/sleuthkit/sleuthkit/blob/develop/tsk/fs/hfs.cpp
- ★ [s12] The Sleuth Kit source, `tsk/fs/hfs_dent.cpp`.
  https://github.com/sleuthkit/sleuthkit/blob/develop/tsk/fs/hfs_dent.cpp
- ★ [s13] The Sleuth Kit source, `tsk/fs/hfs_journal.cpp`.
  https://github.com/sleuthkit/sleuthkit/blob/develop/tsk/fs/hfs_journal.cpp
- ★ [s14] OSDFCon 2010, “Mac Forensic Tools Using The Sleuth Kit.”
  https://www.osdfcon.org/presentations/2010/joyce-mac-forensics-sleuthkit.pdf
- ★ [s15] The Sleuth Kit, `tsk_recover(1)`.
  https://sleuthkit.org/sleuthkit/man/tsk_recover.html
- ★ [s16] The Sleuth Kit, `fls(1)`.
  https://www.sleuthkit.org/sleuthkit/man/fls.html
- ★ [s17] Debian, `blkls(1)`.
  https://manpages.debian.org/unstable/sleuthkit/blkls.1.en.html
- ★ [s18] Apple, “Technical Note TN1150: HFS Plus Volume Format.”
  https://developer.apple.com/library/archive/technotes/tn/tn1150.html
- [s19] UFS Explorer, “File system and data recovery chances — HFS+.”
  https://www.ufsexplorer.com/articles/os-data-recovery-specifics
- [s20] R-Tools, “Data Recovery on HFS/HFS+ File System.”
  https://www.r-studio.com/Linux_Help/data_recovery_on_hfshfs_file_s.html
- ★ [s21] DFRWS, “Using the HFS+ Journal For Deleted File Recovery.”
  https://dfrws.org/presentation/using-the-hfs-journal-for-deleted-file-recovery
- ★ [s22] Apple Support, “Mac startup key combinations.”
  https://support.apple.com/en-us/102603
- ★ [s23] Apple Support, “Transfer files between two Mac computers using target disk mode.”
  https://support.apple.com/en-tm/guide/mac-help/mchlp1443/mac
- ★ [s24] Roderick W. Smith, “Installing and Uninstalling rEFInd.”
  https://www.rodsbooks.com/refind/installing.html
- [s25] Ventoy issue #3733, “Regression: Ventoy 1.1.17 cannot boot on MacBook Pro A1502 /
  MacBookPro11,1.”
  https://github.com/ventoy/Ventoy/issues/3733
- [s26] Ventoy issue #3774, “Ventoy 1.1.14 and later does not boot on Macs if secure boot
  support is enabled.”
  https://github.com/ventoy/Ventoy/issues/3774
- [s27] Gentoo Wiki, “Apple Macbook Pro Retina (early 2013).”
  https://wiki.gentoo.org/wiki/Apple_Macbook_Pro_Retina_(early_2013)
- [s28] Gentoo Wiki, “Apple Macbook Pro Retina 15-inch (early 2013).”
  https://wiki.gentoo.org/wiki/Apple_Macbook_Pro_Retina_15-inch_(early_2013)
- [s29] DeepakNess, “Fedora 44 Wi-Fi and Webcam fix on Mid-2014 MacBook Pro.”
  https://deepakness.com/raw/fixing-wifi-fedora-macbook
- [s30] OWC, “Envoy Pro User Guide.”
  https://eshop.macsales.com/manuals/envoy-pro-user-guide
- ★ [s31] Apple Support, “Prepare for APFS in macOS High Sierra.”
  https://support.apple.com/en-us/100765
- ★ [s32] Apple, “Apple File System Guide – FAQ.”
  https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/APFS_Guide/FAQ/FAQ.html
- ★ [s33] apfs-fuse, README.
  https://github.com/sgan81/apfs-fuse
- ★ [s34] The Sleuth Kit, “History.”
  https://www.sleuthkit.org/sleuthkit/history.php
- [s35] Plugable, “Trim an SSD in macOS.”
  https://kb.plugable.com/data-storage/trim-an-ssd-in-macos
- [s36] Rossmann Group, “Can You Recover Data After TRIM?”
  http://rossmanngroup.com/problems/can-you-recover-data-after-trim
- ★ [s37] SystemRescue, “Packages included in SystemRescue.”
  https://www.system-rescue.org/Detailed-packages-list/
- ★ [s38] SystemRescue, “System tools.”
  https://www.system-rescue.org/System-tools
- [s39] Rescuezilla issue #112, “How to use the File Recovery / Undelete functionality?”
  https://github.com/rescuezilla/rescuezilla/issues/112
- ★ [s40] Kaisen Linux, “Tools.”
  https://kaisenlinux.org/documentation/tools.html
- ★ [s41] Mariposa Rescue Disk, SourceForge project files/readme.
  https://sourceforge.net/projects/mariposarescue/files/MariposaRescue
- [s42] RevLix, README.
  https://github.com/cektor/RevLix
- ★ [s43] RecoverPy, README.
  https://github.com/PabloLec/RecoverPy
- ★ [s44] libfvde, README.
  https://github.com/libyal/libfvde
- [s45] HFSleuth, newosxbook.
  https://newosxbook.com/tools/hfsleuth.html
- ★ [s46] OpenSuperClone Wiki.
  https://github.com/ISpillMyDrink/OpenSuperClone/wiki
- ★ [s47] Red Hat, “Relax-and-Recover (ReaR).”
  https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/7/html/system_administrators_guide/ch-relax-and-recover_rear
- [s48] LinuxLinks, “Mondo Rescue.”
  https://www.linuxlinks.com/Mondo
- ★ [s49] Debian, `fsck.hfsplus(8)`.
  https://manpages.debian.org/testing/hfsprogs/fsck.hfsplus.8
- ★ [s50] Ubuntu, `hpls(1)`.
  https://manpages.ubuntu.com/manpages/jammy/man1/hpls.1.html
- [s51] Apple StackExchange, “Do MacBooks support NVMe SSD drives…?”
  https://apple.stackexchange.com/questions/287644/do-macbooks-support-nvme-ssd-drives-via-the-use-of-a-sintech-adapter
- ★ [s52] GNU Project, “GNU ddrescue.”
  https://www.gnu.org/software/ddrescue
- ★ [s53] The Sleuth Kit, “File and Volume System Analysis.”
  https://www.sleuthkit.org/sleuthkit/desc.php
