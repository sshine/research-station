# Audit

## Run summary

- **Job:** `01a1b7ef-63cf-753a-ba84-6e6a58a4dcca`
- **Pipeline depth:** quick
- **Pipeline result:** completed with audit findings; 12 queries, 44 sources, 4 claims, 2/3 subquestions covered, stop reason `max_rounds,budget_queries`
- **Manual review:** 10 additional sources read and quoted; 3/3 subquestions covered
- **Final report:** `report.md` and `report.html`, both with 10 numbered references in the same order

## Coverage after manual review

| Subquestion | Sources read | Status |
|---|---:|---|
| 1. What is the cyberweapon called, and what technical evidence shows it was designed to sabotage air-gapped Iranian nuclear centrifuge operations? | 7 | Covered |
| 2. What reporting or documents support US/NSA/CIA involvement in Stuxnet and the associated Olympic Games programme? | 5 | Covered, with caveats |
| 3. What evidence supports Israeli involvement in Stuxnet, including reported roles for Unit 8200, Mossad, or a joint US–Israel programme? | 4 | Covered, with caveats |

## Citation validation

- **Markdown inline citations:** first-citation order is `1 2 3 4 5 6 7 8 9 10`.
- **HTML inline citations:** first-citation order is `1 2 3 4 5 6 7 8 9 10`.
- **Reference entries:** 10 in Markdown, 10 in HTML; all are cited at least once.
- **HTML ids:** `ref-1` … `ref-10` are unique; every `href="#ref-n"` resolves.
- **HTML assets:** no external scripts, stylesheets, images, or fonts; CSS is inline and the document is self-contained.
- **Accessibility:** skip link, `header`/`main`/`footer` landmarks, section headings, table caption and `scope` headers, dark-mode via `prefers-color-scheme`, responsive layout, print styles.

## Findings and caveats

1. **The name is not in dispute.** Stuxnet is the worm; Operation Olympic Games is the programme. The technical literature and the journalism agree.
2. **The "nuclear power plant" premise is wrong.** The target was the Natanz enrichment plant. The CRS's December 2010 summary uses "nuclear power plant" loosely; later, more detailed reporting and the IAEA-based ISIS analysis identify Natanz. The report states this correction explicitly.
3. **The "NSA-funded" premise is imprecise.** The sources establish NSA development, CIA covert-action authority, Israeli participation, and presidential authorization. They do not document a separate NSA funding line. The report says "NSA-built" or "NSA-linked" is more precise.
4. **Attribution is journalistic, not official.** The US government has never formally acknowledged the operation. The core attribution comes from unnamed officials quoted by the New York Times and Washington Post in June 2012, plus later Wired and CBS reporting. The report labels this and does not present it as a declassified fact.
5. **The centrifuge count varies.** "Nearly 1,000" (NYT, WaPo), "about a thousand" (Wired), "approximately 1,000" (IEEE), and "about 1,000 IR-1 centrifuges replaced" (ISIS, explicitly cautious). The report gives the range and the different levels of confidence.
6. **One pipeline claim was not reused.** The pipeline's Bismarck Brief source for Unit 8200 was replaced by the NYT's direct reporting, which is stronger and contemporaneous with the 2012 attribution.

## Result

**Pass with caveats.** The bundle answers the question, corrects the two inaccurate premises, and keeps the attribution uncertainty visible.
