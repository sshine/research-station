# Check the filesystem first: HFS+, APFS, FileVault, TRIM

The biggest error in the question is the word “presumably HFS+”. A 2013–2014 MacBook
shipped with an SSD, and macOS converts SSD boot volumes to APFS automatically.

## APFS conversion

Apple: “When you install macOS High Sierra on the Mac volume of a solid-state drive (SSD)
or other all-flash storage device, that volume is automatically converted to APFS.
Fusion Drives, traditional hard disk drives (HDDs), and non-Mac volumes aren’t converted.
You can’t opt out of the transition to APFS.” [s31]

So:

- A 2013–2014 MacBook that ever ran High Sierra (10.13) or later on its internal SSD is
  almost certainly **APFS**, not HFS+.
- HFS+ survives on: a machine never upgraded past Sierra; an external HDD; an external
  SSD/HDD formatted HFS+; a volume manually reformatted HFS+ (possible on external media).
- If the Mac is running Mojave/Catalina/Big Sur on the internal SSD, it is APFS.

## How to identify without writing

On macOS (read-only commands):

```
diskutil list
diskutil info disk0s2        # look at "Type (Bundle)", "File System Personality", "Encrypted"
diskutil apfs list           # if APFS, shows container/volumes/snapshots
diskutil info disk0s2 | grep -i -E 'Type|File System|Encrypted|TRIM'
```

On Linux, against the image or device:

```
lsblk -f
blkid
file -s /dev/sdX2
mmls /dev/sdX                 # partition layout, offset for fsstat
fsstat -o <sector-offset> /dev/sdX   # TSK will report hfs vs apfs
```

TSK’s `fsstat` autodetects HFS+ and APFS (APFS support since 4.8). [s34] If it reports
`apfs`, the HFS+ tools in the roundup are the wrong branch.

## FileVault 2 / CoreStorage (HFS+ era)

On Lion through Sierra, FileVault 2 wraps the HFS+ volume in a CoreStorage logical volume.
The raw partition looks like “Apple CoreStorage”, not HFS+. `libfvde` can unlock it with the
user password or recovery key:

```
fvdeinfo disk.img
fvdemount -o <partition-offset-bytes> -p '<password>' disk.img /mnt/fvde
# or: -r <recovery-key>, or -e EncryptedRoot.plist.wipekey
```

Then scan `/mnt/fvde/fvde1`. `libfvde` is experimental, supports Lion through Catalina
CoreStorage, and does not support multiple physical volumes or volumes ≥1 TiB. [s44]

On APFS, encryption is native APFS; use `apfs-fuse -r <password-or-PRK>` or the libfsapfs
tooling. `apfs-fuse` supports software-encrypted volumes and Fusion drives (read-only). [s33]

## TRIM: the destructive caveat

Apple enables TRIM by default on Apple-supplied internal SSDs. When a file is deleted, the
SSD is told that the blocks are no longer needed; the controller may unmap them and return
zeros on subsequent reads. The exact timing depends on the controller, firmware and garbage
collection, but the practical recovery window on an internal Apple SSD is very small — often
zero. [s35][s36]

Consequences:

- If the deleted file was on the **internal SSD**, a deep scan may find only zeros, no
  matter which tool you use.
- If the file was on an **external USB hard drive**, TRIM usually does not pass through
  USB, so the old blocks remain and recovery is much more likely. [s36]
- If the internal SSD is a **third-party** drive and `trimforce` was never enabled, TRIM may
  be off, improving the odds. [s35]
- A **FileVault** volume does not change the TRIM problem; encryption is not what makes the
  deleted blocks disappear. [s36]

This is why the report leads with “check TRIM before spending hours on a scan”.

## APFS snapshots: the best APFS recovery path

APFS is copy-on-write and supports snapshots. A snapshot taken before the deletion still
contains the file, with its original name and path. This is not carving; it is a
point-in-time view.

- On macOS: `tmutil listlocalsnapshots /` and `diskutil apfs listSnapshots disk3s1`.
- On Linux, from an image: `apfs-fuse` can list and mount snapshots (`snap=<xid>`, `xid=<n>`)
  and can mount FileVault volumes with `-r`. [s33]
- TSK’s APFS support can parse snapshots, though its APFS support is described as limited
  and encryption support is incomplete. [s34]

If snapshots exist, exhaust them before carving. APFS deleted-file recovery below the
snapshot layer is much harder than HFS+ because APFS uses an object map and copy-on-write
metadata; the deleted file’s object map entry is removed, and there is no HFS+-style
catalog record to scan. Carving is the fallback.

## HFS+ compression, again

On HFS+, compressed files have an empty data fork and their content in
`com.apple.decmpfs` and/or the resource fork. On APFS the same mechanism exists. A carver
that only looks at the data fork will miss them. TSK 4.0+ handles HFS+ `com.apple.decmpfs`;
`apfs-fuse` supports ZLib and LZVN transparent decompression but not every compression
method. [s10][s33]

## Quick decision tree

```
Is the volume APFS?
├─ yes → snapshots first (tmutil/diskutil, apfs-fuse -s list / snap=)
│         then TSK APFS / apfs-fuse for live files
│         then PhotoRec carving on the image
│         HFS+ tools (hfsprescue, TestDisk, hfsprogs) do not apply
└─ no → is it CoreStorage/FileVault?
          ├─ yes → libfvde unlock → then HFS+ path
          └─ no → HFS+ path:
                   image → hfsprescue (metadata) → TSK (live/known CNID)
                   → blkls + PhotoRec/Foremost/Scalpel (carving)
```
