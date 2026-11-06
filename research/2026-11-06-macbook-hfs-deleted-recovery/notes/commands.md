# Reproducible command sequence

All commands assume the suspect disk is `/dev/sdX` and the evidence disk is mounted at
`/mnt/evidence`. **Never write to the suspect disk.** Replace `sdX` with the real device
and check with `lsblk` first.

## 0. Record the device and its identity

```
lsblk -o NAME,SIZE,MODEL,SERIAL,FSTYPE,MOUNTPOINTS
sudo smartctl -a /dev/sdX        # if SMART is available
sudo blockdev --getsize64 /dev/sdX
```

## 1. Image the disk (or the partition)

Whole disk is better because the partition table and any CoreStorage/APFS container
metadata may matter.

```
sudo ddrescue -d -r3 /dev/sdX /mnt/evidence/mac.img /mnt/evidence/mac.map
sha256sum /mnt/evidence/mac.img | tee /mnt/evidence/mac.img.sha256
```

- `-d` direct I/O; `-r3` retry bad areas three times. For a failing drive, use
  OpenSuperClone or a multi-pass ddrescue strategy instead.
- If the drive is healthy, a plain `dd`/`dc3dd` is fine; ddrescue is still preferable
  because it is resumable and logs progress. [s52][s46]

## 2. Find the filesystem and its offset

```
mmls /mnt/evidence/mac.img
fsstat /mnt/evidence/mac.img                 # if the image is a single partition
fsstat -o <sector-offset> /mnt/evidence/mac.img
```

`fsstat` reports `hfs` or `apfs` (TSK 4.8+). The `-o` is a **sector** offset, not bytes. [s34]

## 3. Optional: mount read-only (only on the image, never the original)

```
sudo losetup --find --show --read-only --offset $((<sector-offset>*512)) /mnt/evidence/mac.img
sudo mount -t hfsplus -o ro /dev/loopN /mnt/hfs
```

If the kernel refuses because the volume is journaled/dirty, do not “force” it on the
original. Use TSK or `hfsprescue` to read the structures instead. [s33][s49]

## 4. HFS+ metadata deep scan with hfsprescue

```
mkdir -p /mnt/evidence/hfsp && cd /mnt/evidence/hfsp
hfsprescue -s1 /mnt/evidence/mac.img -o $((<partition-start-bytes>))
hfsprescue -s2
hfsprescue --list | less
hfsprescue --csv files.csv
hfsprescue -s3 /mnt/evidence/mac.img -o $((<partition-start-bytes>)) --alternative
hfsprescue -s4
hfsprescue -s5
hfsprescue -s6
```

- `-o` here is the partition start in **bytes** (not sectors).
- `-o` sets the start but not the end: on a whole-disk image the scan continues past the
  HFS+ partition. For a bounded, faster scan, image the partition itself
  (`ddrescue /dev/sdX2 partition.img partition.map`).
- `-s1` scans every allocation block for catalog records; on a large or failing volume it
  is slow. Image first, then run it on the image. [s09][s06]
- `--alternative` preserves deleted/older versions when a file with the same name already
  exists; the manual warns they may be corrupted. [s07]
- `--one-file <image> <number>` restores a single file from the `--list` output.
- If the volume header is damaged, `--find-vh`, `--find-avh`, `--find-eof`,
  `--extract-eof`, `--extract-vh` are the recovery-of-last-resort options. [s07]

## 5. TSK triage (live files, known CNIDs, unallocated space)

```
fls -f hfs -o <sector-offset> -r -p /mnt/evidence/mac.img | less
fls -f hfs -o <sector-offset> -r -d -p /mnt/evidence/mac.img | less   # usually empty on HFS+
istat -f hfs -o <sector-offset> /mnt/evidence/mac.img <cnid>
icat  -f hfs -o <sector-offset> /mnt/evidence/mac.img <cnid> > recovered.bin
```

`fls -d` is not a deleted-file finder on HFS+: TSK only walks the live catalog B-tree and
marks every entry it reaches as allocated. [s11][s12] `icat` still works if you have a CNID
from another source (an extent record, a journal hit, a stale catalog record).

Dump unallocated space for carving:

```
blkls -f hfs -o <sector-offset> -A /mnt/evidence/mac.img > /mnt/evidence/mac-unalloc.img
```

`blkls` defaults to unallocated blocks; `-A` makes it explicit; `-a` gives allocated; `-e`
gives everything. [s17]

## 6. Carving

On the whole image or on the unallocated dump:

```
mkdir -p /mnt/evidence/photorec
photorec /d /mnt/evidence/photorec /mnt/evidence/mac.img
```

- In the menu, choose the HFS+ partition (or “No partition” for a whole-disk scan).
- For the filesystem type, choose **Other** unless it is ext2/3/4; HFS+ is not ext.
- Choose **Whole** if “Free” is not offered; PhotoRec’s free-space mode is primarily a
  FAT/NTFS option. [s05]
- File Opt: deselect everything and enable only the types you need (e.g. jpg, pdf, docx,
  zip, mp4). This cuts noise and runtime.

Foremost / Scalpel:

```
foremost -t all -i /mnt/evidence/mac.img -o /mnt/evidence/foremost
scalpel -c /etc/scalpel/scalpel.conf -o /mnt/evidence/scalpel /mnt/evidence/mac.img
```

Both are signature carvers; they do not read HFS+ metadata. [s01]

## 7. Text/pattern search

```
recoverpy /dev/loopN          # interactive; pick the partition, enter a distinctive string
# or, non-interactively:
grep -abo --binary-files=text 'distinctive phrase' /mnt/evidence/mac.img | head
```

RecoverPy scans raw bytes and lets you inspect adjacent blocks; it does not reconstruct
files or names. [s43]

## 8. Repair attempts (only on a copy)

```
fsck.hfsplus -n /mnt/evidence/mac.img          # read-only check
fsck.hfsplus -r /mnt/evidence/mac.img          # rebuild catalog (writes!)
```

`fsck.hfsplus -r` rebuilds the catalog B-tree and can move orphaned files/directories into
`lost+found`. It is a repair, not an undelete, and it will not bring back catalog records
that are gone. Run it only on a disposable copy. [s49]

## 9. FileVault 2 / CoreStorage

```
fvdeinfo /mnt/evidence/mac.img
fvdemount -o $((<partition-start-bytes>)) -p '<password>' /mnt/evidence/mac.img /mnt/fvde
# or -r <recovery-key>, or -e EncryptedRoot.plist.wipekey
```

Then run hfsprescue/TSK/PhotoRec against `/mnt/fvde/fvde1`. [s44]

## 10. APFS

```
apfsutil /mnt/evidence/mac.img                 # list volumes and snapshot XIDs (if built)
apfs-fuse -o ro,vol=1 /mnt/evidence/mac.img /mnt/apfs
apfs-fuse -o ro,vol=1,snap=<xid> /mnt/evidence/mac.img /mnt/snap
apfs-fuse -r '<password-or-PRK>' -o ro,vol=1 /mnt/evidence/mac.img /mnt/apfs
```

`apfs-fuse` is read-only, supports snapshots and software-encrypted volumes, and does not
support every compression method. [s33]
