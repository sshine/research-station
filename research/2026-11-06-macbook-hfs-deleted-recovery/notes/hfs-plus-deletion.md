# HFS+ deletion mechanics and what survives

## On-disk structures

- **Volume header** at byte offset 1024 from the start of the HFS+ volume; a copy (alternate
  volume header) sits 1024 bytes before the end. It holds the allocation block size, the
  catalog/extents/attributes file fork descriptors, and the journal info block. [s18]
- **Catalog B-tree** (`$Catalog`): the directory. Leaf records are folder records (type 1),
  file records (type 2), folder thread records (3), and file thread records (4). The key is
  `parentID` + Unicode name; the data carries the CNID, timestamps, permissions, and the
  first eight extents of each fork. [s18][s10]
- **Extents overflow B-tree** (`$ExtentsFile`): additional extents for forks with more than
  eight extents, keyed by `fileID`/`forkType`/`startBlock`. [s18]
- **Attributes B-tree** (`$AttributesFile`): extended attributes, including
  `com.apple.decmpfs` for HFS+ compression. [s18][s10]
- **Allocation file** (`$AllocationFile`): the bitmap of allocated/free allocation blocks. [s18]
- **Journal** (`$Journal` / journal info block): transactional record of metadata changes,
  used to make the volume consistent after an unclean unmount. [s18]

## What deletion does

1. macOS removes the file's catalog record from the catalog B-tree and rewrites/rebalances
   the affected nodes. [s19][s20]
2. The file's allocation blocks are marked free in the allocation bitmap. [s19]
3. The file's data blocks are **not** erased at that moment; they remain until reallocated
   and overwritten. [s19][s20]
4. Extent records in the extents overflow B-tree can remain keyed by the old CNID. The
   catalog record that would let you find that CNID is gone, but a raw scan can still find
   the extent record. [s18][s06]
5. The journal may still contain the catalog transaction for a short time, until it wraps. [s19][s21]

This is the crucial difference from FAT/NTFS: HFS+ does not leave a directory entry marked
“deleted”. It removes the record. `fls -d` therefore has nothing to list on a healthy HFS+
volume, and TestDisk's undelete code (which relies on a deleted directory entry) does not
apply. [s02][s03][s12]

## Where deleted records can still be found

- **Unused catalog B-tree nodes.** A B-tree node that was split/merged or freed can retain
  the old record bytes until the node is reused. A raw scan of every allocation block for
  catalog key/data patterns can recover names, parent IDs, CNIDs, sizes and extents. This is
  what `hfsprescue` does. [s09][s06]
- **Extents overflow records.** Orphaned extent records can describe blocks that the
  catalog no longer references. `hfsprescue --find-eof` / `--extract-eof` exist specifically
  to recover the extents overflow file when the volume header is damaged. [s07]
- **Journal.** The DFRWS work by Burghardt & Carrier showed that recent deletions can be
  reconstructed from journal transactions because they retain file attributes and extents.
  No maintained open-source implementation was found; TSK's HFS journal functions are
  stubs. [s21][s13]
- **Data blocks.** Even with no metadata, the bytes remain until overwritten. Carvers find
  them by signature. [s04]

## HFS+ compression

Compressed files have an **empty data fork**. The content lives in the
`com.apple.decmpfs` extended attribute and/or the resource fork, compressed with zlib,
LZVN or LZFSE. A carver that only looks at the data fork sees nothing. TSK 4.0+ understands
`com.apple.decmpfs` and decompresses on `icat`; `hfsprescue` advertises HFS+ compression
(resource fork) support. PhotoRec does not. [s10][s06][s04]

## Resource forks and hard links

- Resource forks are a second fork; most user files have none, but some applications store
  data there. A carver that only extracts the data fork loses it. TSK exposes the resource
  fork as attribute `4353-1`. [s10]
- HFS+ hard links are files in `/^^^^HFS+ Private Data` named `iNode<cnid>`; directory hard
  links (used by Time Machine) live in `/HFS+ Private Directory Data<CR>` as `dir_<n>`.
  A recovery tool that ignores these will produce missing or duplicated content. [s10]

## Timestamps

HFS+ timestamps are seconds since 1904-01-01, not the Unix epoch. `hfsprescue` subtracts
`2082844800` seconds when printing dates. A custom parser must do the same. [s09]

## Practical consequence

There are three different “deep scans”, and they recover different things:

| Layer | What it finds | Names/paths? | Example |
|---|---|---|---|
| Catalog-record scan | Stale file/folder records in unused nodes, extents, journal | Yes, if the record survives | `hfsprescue` |
| Filesystem-aware listing | Live catalog only | Yes | TSK `fls`, `hfsplus` |
| Data carving | File content by signature | No | PhotoRec, Foremost, Scalpel |

A tool that only does the middle row is not a deleted-file tool on HFS+.
