# Research brief

- **Original request:** Using the tools in the LinuxLinks “18 Best Free and Open Source
  Linux Data Recovery Tools” roundup (ddrescue, The Sleuth Kit, SystemRescue, TestDisk,
  Relax-and-Recover, RecoverPy, Mondo Rescue, PhotoRec, Rescuezilla, OpenSuperClone,
  Mariposa Rescue Disk, safecopy, Redo Rescue, Foremost, DDRescue-GUI, Scalpel,
  ext4magic, RevLix), work out how to boot a Live Linux USB on a 2013–2014 Intel
  MacBook and recover deleted files from an HFS+ filesystem. The specific question is
  which tools (from that list or elsewhere) can perform an **HFS+ deep scan** — find
  files that are no longer listed in a directory but whose data is still on disk.
- **Sharpened question:** For a 2013–2014 MacBook (MacBookPro11,x / MacBookAir6,x,
  64-bit EFI, no T2), what is the reliable boot path into a Linux live environment, what
  does “deleted” mean at the HFS+ on-disk level, and which open-source Linux tools can
  actually recover HFS+ files that are absent from the catalog B-tree? Which of the
  roundup tools are imaging tools, which are carve-only, which are filesystem-aware,
  and which are simply irrelevant to HFS+?
- **Depth:** thorough (manual evidence collection; built-in web search/fetch plus two
  provider-backed Tavily searches).
- **Created:** 2026-11-06.
- **Output:** `research/2026-11-06-macbook-hfs-deleted-recovery`
- **Budget:** 30 queries, 50 sources, 120 credits, 1 800 s wall time (actual usage in
  `audit.md`).

## Scope

1. **Establish the actual filesystem first.** A 2013–2014 MacBook shipped with an SSD.
   macOS High Sierra (10.13) and later automatically and unavoidably convert an SSD boot
   volume to APFS; HFS+ survives on pre-High-Sierra installs, external HDDs, and
   manually formatted volumes. The report must branch on HFS+ vs APFS rather than assume
   HFS+.
2. **Establish the HFS+ deletion mechanics.** Volume header, catalog B-tree, extents
   overflow B-tree, attributes B-tree, allocation bitmap, journal. What is removed on
   delete, what survives, and for how long.
3. **Evaluate every roundup tool against HFS+ deep scan.** Separate:
   imaging/cloning; partition repair; live-distro packaging; carve-only recovery;
   filesystem-aware recovery; unrelated backup/DR tools.
4. **Identify tools that actually scan for deleted HFS+ metadata.** Verify with primary
   documentation and, where possible, source code: `hfsprescue`, The Sleuth Kit,
   PhotoRec, TestDisk, `fsck.hfsplus`, HFSleuth, libfvde, APFS tools.
5. **Establish the Mac boot path.** Apple startup keys, firmware password, UEFI vs
   legacy, GPT/FAT32 ESP, rEFInd, `bless`, Ventoy regressions, GPU/Wi-Fi/keyboard/USB
   quirks, Target Disk Mode, and removing the Apple PCIe SSD.
6. **Record the destructive caveats.** TRIM on Apple SSDs, FileVault 2/CoreStorage,
   APFS snapshots, HFS+ compression/resource forks, fragmented files, and the difference
   between what a tool claims and what it can actually do.

## Out of scope

- Recovering data from a physically failing drive beyond the imaging step; that is a
  hardware-cleanroom problem once `ddrescue`/OpenSuperClone cannot read the sectors.
- Windows-only or commercial recovery products (Disk Drill, R-Studio, UFS Explorer,
  Hetman) except as comparison points where they clarify what “deep scan” means.
- Guaranteeing recovery. HFS+ deletion is destructive by design; the report states
  probabilities and failure modes, not promises.
- Modifying the Mac’s firmware, bypassing a firmware password, or circumventing
  FileVault without the owner’s password/recovery key.
