# Tool matrix: what each roundup tool is actually for

The LinuxLinks roundup mixes six different jobs under “data recovery”. [s01] Classifying
them is the whole answer to “which tool can deep-scan HFS+”.

| Tool | Job | HFS+ deleted-file scan? | Evidence / limit |
|---|---|---|---|
| **ddrescue** | Imaging / cloning | No | Copies a device to an image, skips bad areas, resumes from a mapfile. It does not parse a filesystem. [s52] |
| **The Sleuth Kit (TSK)** | Forensic filesystem analysis | **No deleted HFS+ enumeration** | HFS+ is supported for reading live catalog entries, resource forks, xattrs, hard links and compressed files. But TSK marks every reachable catalog entry `TSK_FS_NAME_FLAG_ALLOC`, starts traversal at the catalog root, and its HFS journal functions are stubs. `icat` works only if you already know a CNID. [s10][s11][s12][s13] |
| **SystemRescue** | Live rescue distro | N/A (packaging) | Includes `sleuthkit 4.15.0`, `testdisk 7.2`, `ddrescue`, `foremost`. [s37] |
| **TestDisk** | Partition table / boot sector repair | **No** | Undelete is explicitly limited to FAT, exFAT, NTFS and ext2. HFS+ support is boot-sector backup restore. [s02][s03] |
| **Relax-and-Recover (ReaR)** | Bare-metal disaster recovery | No | Creates a rescue image and restores a system; not file-level recovery. [s47] |
| **RecoverPy** | Raw byte-pattern search | Partial, data-level only | Streams the raw partition looking for a string; shows adjacent blocks; explicitly does not interpret filesystem structures or restore filenames. [s43] |
| **Mondo Rescue** | Backup / disaster recovery | No | Backs up a Linux system to tape/CD/NFS/disk; not an undelete tool. [s48] |
| **PhotoRec** | File carving | **Yes, data-level; no names** | Ignores the filesystem and carves by signature. HFS+ is listed among supported filesystems, but fragmented files, resource forks and decmpfs-compressed files are lost or broken. [s04][s05] |
| **Rescuezilla** | Live distro / imaging GUI | N/A (packaging) | Bundles TestDisk and PhotoRec; the project itself calls PhotoRec the “photograph deep scan” and TestDisk the “file recovery” tool. No HFS+ undelete. [s39] |
| **OpenSuperClone** | Imaging / cloning | No | Sector-by-sector clone or image of failing media, multi-phase and resumable. [s46] |
| **Mariposa Rescue Disk** | Live rescue distro | N/A (packaging) | Debian 12 live with `sleuthkit`, `testdisk`, `photorec`, `ddrescue` and many other tools. [s41] |
| **safecopy** | Imaging | No | Copy utility that ignores read errors; no filesystem parser. [s01] |
| **Redo Rescue** | Live distro / backup | N/A (packaging) | TestDisk, PhotoRec and ddrescue under a GUI. [s01] |
| **Foremost** | File carving | **Yes, data-level; no names** | Recovers files from headers/footers; configuration-driven; no HFS+ metadata. [s01] |
| **DDRescue-GUI** | Frontend | No | GUI for ddrescue. [s01] |
| **Scalpel** | File carving | **Yes, data-level; no names** | Config-driven carving; no HFS+ metadata. [s01] |
| **ext4magic** | ext2/3/4 recovery | **No** | ext-family only; cannot read HFS+. [s01] |
| **RevLix** | GUI wrapper | No new engine | PyQt5 GUI around PhotoRec, TestDisk, ddrescue, Foremost and SMART. It inherits PhotoRec's carving limits and TestDisk's lack of HFS+ undelete. [s42] |

## Tools outside the roundup that matter for HFS+

| Tool | Job | HFS+ deleted-file scan? | Evidence / limit |
|---|---|---|---|
| **hfsprescue** | HFS+ metadata recovery | **Yes — the only turnkey open-source option found** | Scans every block of the partition for catalog key/data patterns; recovers names, parent IDs, CNIDs, sizes and extents; supports HFS+ compression; `--alternative` restores deleted/older versions. GPL-2.0, Linux/macOS/FreeBSD, v3.6 (2023). [s06][s07][s08][s09] |
| **HFSleuth** | HFS+/HFSX inspection | Manual only | Free binary (not open source) from newosxbook; inspects/traverses volumes and images on Linux, but it is a forensic inspection tool, not a one-click undelete. [s45] |
| **hfsplus / hfsprogs / hfsutils** | Mount, list, copy, fsck | No | `hpls`, `hpmount`, `hpcopy` access live files. `fsck.hfsplus -r` rebuilds the catalog and can move orphans to `lost+found`, but that is repair, not a deleted-file scan. [s49][s50] |
| **libfvde** | FileVault 2 / CoreStorage | Prerequisite | `fvdeinfo` / `fvdemount` unlock CoreStorage volumes with a password or recovery key; scan the resulting `fvde1` device. [s44] |
| **apfs-fuse** | APFS read-only mount | APFS branch | Mounts APFS read-only, including snapshots (`snap=`, `xid=`) and FileVault (`-r`). [s33] |
| **TSK APFS** | APFS forensic analysis | APFS branch | TSK 4.8+ has limited APFS support; snapshots parseable; encryption incomplete. [s34] |
| **HFS+ forensic (Rust)** / **libfshfs** | Read-only parsers | Scripting only | Useful for custom carving/parsing, not turnkey recovery. [s01] |

## Key distinction

- **Filesystem-aware recovery** preserves names/paths because it reads catalog records.
  On HFS+, only `hfsprescue` does this for *deleted* records in the open-source set.
- **Carving** recovers content but loses names, and breaks on fragmentation and HFS+
  compression.
- **Imaging** is not recovery; it is the correct first step before either.
