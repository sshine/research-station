# Audit

## Pipeline run

- **Job:** `01a1ac10-1114-77fa-b7af-186b9164919b`
- **Question:** "Shelly Plug S Gen3 alternatives smart plugs Denmark local control power monitoring"
- **Depth:** thorough · **Provider:** Tavily · **Planner:** rule-based-v1 ·
  **Synthesizer:** extractive-v1
- **Result:** completed-with-audit-findings
- **Usage:** 8 queries, 29 sources, 4 claims, 16 credits
- **Pipeline audit:** 4/4 claims supported by exact quotes; subquestion coverage 2/4;
  `passed: false` (see `run.json` and the table below).

The pipeline's rule-based planner produced generic subquestions (e.g. "what is … 
definition"), so its extractive synthesis covered only 2 of 4 subquestions. That is a
pipeline-planner limitation, not a contradiction in the evidence.

| Pipeline subquestion | Sources | Claims |
|---|---:|---:|
| Overview and definitions | 9 | 2 |
| Primary sources and documentation | 5 | 0 |
| Limitations, risks, criticisms | 0 | 0 |
| Recent developments and comparisons | 15 | 2 |

## Final report audit

The final `report.md` is a separate, human synthesis written after the pipeline run. It
does not reuse the pipeline's 4 claims as its structure; it uses the pipeline sources plus
additional primary sources retrieved on 2026-11-05.

### Source hierarchy

1. **Primary manufacturer pages** for specifications and status:
   - Shelly product pages, knowledge base, support comparison, "Powered by Shelly"
     support article.
   - Eve, Aqara, frient, Nous, EcoFlow product pages.
2. **Retailer listings** for Danish price/stock:
   - Proshop search and product pages, avXperten category/product pages.
3. **Independent reviews/community reports** only for caveats that manufacturers do not
   document:
   - Sonoff S60ZBTPF: Zigbee2MQTT vs ZHA entity coverage.
   - Matter energy monitoring: Meross/Tapo/Bosch cases where energy data may stay in the
     vendor app.

### Claim checks performed

- **Discontinued status:** verified on Shelly's own S Gen3 product page ("no longer
  produced and sold by Shelly Group") and the recommended successor (M Gen3).
- **Load ratings:** verified against Shelly's comparison table and product pages (S 12 A /
  2500 W; M 13 A / 3000 W; PM 16 A / 3680 W; Outdoor 12 A / 2500 W).
- **PM Gen3 has no relay:** verified in Shelly's support comparison table (`Relay ON/OFF:
  No`) and the product/shop copy.
- **MQTT/local API:** verified in Shelly S and M product pages (`MQTT: Yes`, local web
  interface, scripting).
- **Danish prices/stock:** sampled from Proshop and avXperten on 2026-11-05. Prices are
  snapshots, not guarantees.
- **"Shelly" keyword claim:** checked Proshop `shelly plug` (9 genuine Shelly items),
  Proshop `shelly kompatibel` (Shelly accessories), avXperten's "Shelly Kompatibel"
  EcoFlow listing, EcoFlow's co-brand page, and Shelly's "Powered by Shelly" program.
- **Alternative-brand specs:** Eve EU 11 A / 2500 W; Aqara H2 16 A / 3680 W and
  Matter-over-Thread/Zigbee; Sonoff 16 A / 4000 W and Zigbee caveat; frient 10 A
  continuous / 16 A peak; Osram 10 A / 2300 W; Meross 16 A and app-only energy caveat;
  Nous 10 A / 2400–2500 W and Tasmota/MQTT.

### Disagreements / uncertainty recorded rather than averaged

- **Matter energy monitoring is not uniform.** Some manufacturers expose power/energy to
  third-party Matter controllers; others keep history in their own app. The report says
  this explicitly instead of treating "Matter" as a guarantee of metering.
- **Zigbee energy reporting depends on the coordinator.** The Sonoff review reports
  better entity coverage in Zigbee2MQTT than ZHA; the report flags this.
- **"Local control" has layers.** Pairing/control can be local while energy history is
  cloud/app-only. The report distinguishes these.
- **Stock descriptions differ.** Proshop uses "Fjernlager" rather than "restlager"; the
  report preserves that distinction and the expected stock dates.

### Limitations

- Prices, stock and search-result counts are dynamic and were true only on 2026-11-05.
- The Proshop search page is not a stable catalogue; result counts can change.
- Some retailer descriptions are auto-generated/translated; manufacturer pages were used
  for specifications where possible.
- The report is a shortlist, not an exhaustive market survey. No every-marketplace or
  every-Danish-shop audit was performed.
- No counterfeit products were purchased or lab-tested. The counterfeit discussion is
  limited to what retailer/manufacturer pages and public reports show.

### Result

**Pass with caveats.** Every substantive claim in `report.md` has a linked source; the
main uncertainty is commercial (price/stock) and integration-specific (Matter/Zigbee
metering exposure), both stated in the report.

## Report formats and references (2026-11-06)

- `report.html` was revised to the same numbered-reference contract as the new
  `report.html` in the STOENSE bundle. It is self-contained (inline CSS, no external
  assets), responsive, dark-mode aware, and print-friendly.
- Both `report.md` and `report.html` now use 35 numbered references in order of first
  citation. Markdown uses `[n]` reference-style links with `[n]: <url>` definitions;
  HTML uses `<a href="#ref-n">[n]</a>` anchors and `<li id="ref-n">` entries.
- No separate "HTML prompt" file existed in the bundle or in the available session
  database. The request was therefore interpreted as revising the Shelly HTML report
  (and its Markdown twin) to the new reference format. This is recorded here rather than
  silently assumed.
- Reference metadata: the report cites 35 URLs. Only 4 of those overlap with the 29
  entries in `sources.jsonl`; 31 are additional retailer search/listing pages,
  manufacturer pages, or public reports cited in the report but never retained as ledger
  entries. This is a real storage gap in the bundle, not a claim that the sources are
  weak. Publisher and title were recovered from the report's link text or the page itself,
  and `notes/references.md` records the full reference set and provenance. The reference
  list labels search-result pages as such and does not treat them as authorities on
  product behaviour.
- The user's fallback instruction was to inspect session data when the bundle is
  insufficient. Both available OpenCode databases were queried
  (`~/.local/share/opencode/opencode.db` and `opencode-stable.db`); a case-insensitive
  search of `part.data` for `helly` returned 0 rows. There was therefore no Shelly
  session transcript to recover the missing ledger entries from. The references were
  reconstructed from the report and the cited pages, and the gap is recorded rather than
  hidden.
