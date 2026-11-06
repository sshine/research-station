# Plan v1

- **Plan:** `2026-11-06-macbook-hfs-plan-v1`
- **Topic:** Booting a Linux live USB on a 2013–2014 Intel MacBook and recovering deleted
  HFS+ files with open-source tools.
- **Created:** 2026-11-06
- **Subquestions:** 6

## 1. Is the volume actually HFS+?

- **Source class:** primary
- **Success criterion:** Apple’s APFS conversion rule plus read-only identification
  commands for macOS and Linux.
- **Query seeds:** `macOS High Sierra automatically converts SSD to APFS`,
  `diskutil info file system encrypted TRIM`,
  `Linux detect HFS+ APFS CoreStorage lsblk blkid fsstat`

## 2. What HFS+ removes on delete

- **Source class:** primary / reference
- **Success criterion:** Catalog, extents, allocation and journal roles established from
  Apple TN1150 and forensic sources.
- **Query seeds:** `HFS+ catalog B-tree deleted file extents overflow`,
  `HFS+ journal deleted file recovery DFRWS`,
  `Apple TN1150 HFS Plus volume format catalog journal`

## 3. Classify the 18 roundup tools

- **Source class:** primary
- **Success criterion:** Every tool is classified as imaging, partition repair, carving,
  filesystem-aware recovery, live-distro packaging, or unrelated backup/DR.
- **Query seeds:** `TestDisk undelete HFS+ supported filesystems`,
  `PhotoRec HFS+ file carving`, `Sleuth Kit HFS+ deleted files fls`,
  `ext4magic ext4 only`, `OpenSuperClone cloning imaging`

## 4. Which tools can deep-scan HFS+?

- **Source class:** primary / source code
- **Success criterion:** At least one tool shown, from documentation and source, to scan
  raw HFS+ blocks for catalog records; TSK and PhotoRec limits shown from source/docs.
- **Query seeds:** `hfsprescue restore deleted files HFS+`,
  `hfsprescue source ScanBuffer catalog records`,
  `Sleuth Kit hfs_journal not implemented`,
  `PhotoRec HFS+ filesystem type free space`

## 5. Booting the Mac

- **Source class:** primary / secondary
- **Success criterion:** Boot keys, firmware-password constraint, UEFI USB layout,
  rEFInd fallback, and at least three model-specific hardware quirks.
- **Query seeds:** `Apple startup key combinations Intel Mac Option T`,
  `rEFInd install macOS bless ESP`, `Ventoy MacBookPro11,1 regression`,
  `MacBookPro11,3 nomodeset nouveau`, `BCM4360 broadcom-wl MacBook`

## 6. Alternatives and destructive caveats

- **Source class:** secondary / commercial
- **Success criterion:** SSD-removal path confirmed with a compatibility list; TRIM,
  FileVault and HFS+ compression limits stated with sources.
- **Query seeds:** `OWC Envoy Pro 2013 MacBook PCIe SSD enclosure`,
  `Mac target disk mode Linux Thunderbolt`,
  `Apple SSD TRIM deleted files unrecoverable`,
  `HFS+ compression decmpfs recovery`
