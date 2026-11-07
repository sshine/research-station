# What was the NSA-linked air-gapped attack on Iran's nuclear programme called?

**Short answer:** the cyberweapon was **Stuxnet**, a computer worm built and deployed as part of the classified US–Israeli programme code-named **Operation Olympic Games** [1][2]. It was not a single "NSA-funded" hack in the narrow sense: the strongest reporting says the **National Security Agency developed the weapon**, the **CIA supplied covert-operation authority**, and **Israeli intelligence (including Unit 8200) was a partner** [2][3].

## The name and the operation

Stuxnet was a malicious computer worm first uncovered in June 2010 after it escaped the Iranian plant it was targeting and spread around the world [1][3][4]. The programme that produced it was code-named **Olympic Games**; it began in the Bush administration and was accelerated under President Obama [1][2].

The worm was unusually large and complex for its time — about 500 kilobytes — and infected the software of at least 14 industrial sites in Iran, including a uranium-enrichment plant [4]. It targeted Siemens SIMATIC WinCC and STEP 7 systems, the software used to program and monitor industrial controllers [5][6].

## What it actually targeted — and the "nuclear power plant" correction

The target was **Natanz**, Iran's underground uranium-enrichment plant, not a nuclear power station in the electricity-generating sense [1][2][3]. The New York Times account notes that at the time Iran had only one nuclear power reactor, Bushehr, whose fuel came from Russia, and that the concern was Iran's enrichment capability [1].

Some early coverage speculated that Bushehr was an intended target; the BBC's March 2011 report carried an image caption saying Bushehr was "believed to have been on[e] of the intended targets," while also reporting that Iran denied Stuxnet had caused a shutdown there [7]. The detailed 2012 reporting and the technical evidence centre on Natanz [1][2][5]. A December 2010 Congressional Research Service report loosely described the infected systems as controlling "a nuclear power plant," but also cautioned that the "actual target, if any, is unknown" at that stage [8]. The accurate phrasing is therefore **Iranian nuclear facilities**, specifically the **Natanz enrichment plant**.

## How it crossed the air gap

Natanz's centrifuge controls were isolated from the internet by an air gap [1][3]. Stuxnet crossed that gap through **USB thumb drives** and human access — engineers, maintenance workers, spies and unwitting accomplices — rather than by a remote internet connection [1][3][4][8]. The New York Times reported that "thumb drives turned out to be critical in spreading the first variants of the computer worm" [1]; Wired describes the same physical-access problem and the air gap around the centrifuge controls [3].

## What the worm did

Symantec's reverse-engineering dossier, the primary technical analysis, found that Stuxnet exploited **four zero-day vulnerabilities**, compromised **two digital certificates**, and injected code into Siemens programmable logic controllers while hiding that code from the operator [5]. Its PLC payload monitored the normal operating frequency of the centrifuge drives (between 807 Hz and 1210 Hz), then sent sequences that drove them to 1410 Hz, then 2 Hz, then 1064 Hz — "slowing down or speeding up the motor to different rates at different times" [5]. It also hooked the Siemens monitoring software so that operators saw normal readings while the sabotage was under way [5].

The physical effect is usually described as roughly **1,000 centrifuges damaged**. The New York Times says the last attack in the series "temporarily took out nearly 1,000 of the 5,000 centrifuges Iran had spinning" [1]; the Washington Post says the campaign destroyed "nearly 1,000 of Iran's 6,000 centrifuges" [2]; Wired says "about a thousand" [3]; IEEE Spectrum's later update says "approximately 1,000" [4]. The Institute for Science and International Security, analysing IAEA safeguards data, found that Iran decommissioned and replaced about 1,000 IR-1 centrifuges at Natanz in late 2009 or early 2010, and concluded that the breakage "could have resulted from" Stuxnet — a more cautious formulation than the later journalism [9].

## Who was behind it

The strongest evidence comes from 2012 investigative reporting based on unnamed current and former officials:

- The **Washington Post** reported that "the National Security Agency developed the cyberweapon with help of Israel," that Olympic Games was "a collaborative effort among NSA, the CIA and Israel," and that the CIA "lent its covert operation authority to the program" [2].
- The **New York Times** reported that the worm "had been developed by the United States and Israel," that the N.S.A. and "a secret Israeli unit" developed it, and that Israel's **Unit 8200** had technical expertise "that rivaled the N.S.A.'s" and deep intelligence on Natanz [1].
- **Wired** stated that Stuxnet "was created and built by the NSA in partnership with the CIA and Israeli intelligence in the mid-2000s" [3].

Earlier, in March 2011, the security researcher Ralph Langner publicly claimed that Israel and the United States created Stuxnet and that "there is only one leading source, and that is the United States"; the BBC noted at the time that there was "no hard evidence to link either country to the creation of Stuxnet" [7]. In July 2013, Edward Snowden said "NSA and Israel co-wrote it," but the same CBS report records Langner's doubt that Snowden had evidence for the allegation [10].

The US government has never formally acknowledged the operation [3]. The attribution therefore rests on investigative reporting with anonymous officials, repeated in broad outline by later accounts; Snowden’s statement is a further claim, which Langner disputed. It does not rest on a declassified document, court record, or official admission [1][2][3][10].

## Is "NSA-funded" the right description?

It is close, but it compresses several distinct roles. The sources support **NSA development** of the cyberweapon [2][3], **CIA covert-action authority** and operational planning [2], **Israeli intelligence participation** including Unit 8200 [1][2][3], and **presidential authorization** under Bush and Obama [1][2]. They do not describe a separate, publicly documented "NSA funding line" for Stuxnet. "NSA-built" or "NSA-linked" is more precise than "NSA-funded"; "US–Israeli joint operation" is more precise still [1][2][3].

## Confidence and disagreements

- **High confidence:** the weapon is Stuxnet; the programme is Operation Olympic Games; the target was Natanz; the worm crossed an air gap via removable media and human access; it manipulated Siemens PLCs and damaged roughly 1,000 centrifuges [1][2][3][4][5][9].
- **Moderate confidence:** the exact division of labour among the NSA, CIA, Unit 8200 and other Israeli elements. The 2012 reporting is consistent on a joint programme but differs in emphasis, and all of it relies on unnamed officials [1][2][3].
- **Lower confidence / explicitly disputed:** whether Snowden's "co-wrote it" statement reflects direct knowledge; Langner doubted it [10]. The precise number of destroyed centrifuges also varies (5,000 vs 6,000 installed; "nearly 1,000" vs "about 1,000") [1][2][4][9].
- **Correction to the question's premise:** Stuxnet was used against an enrichment plant, not a nuclear power plant; and the operation was jointly run, not a standalone NSA "funded hack" [1][2][7][8].

## References

1. David E. Sanger, "Obama Order Sped Up Wave of Cyberattacks Against Iran," *The New York Times*, 1 June 2012. https://www.nytimes.com/2012/06/01/world/middleeast/obama-ordered-wave-of-cyberattacks-against-iran.html (accessed 2026-11-07).
2. Ellen Nakashima and Joby Warrick, "Stuxnet was work of U.S. and Israeli experts, officials say," *The Washington Post*, 2 June 2012. https://www.washingtonpost.com/world/national-security/stuxnet-was-work-of-us-and-israeli-experts-officials-say/2012/06/01/gJQAlnEy6U_story.html (accessed 2026-11-07).
3. James Bamford, "NSA Snooping Was Only the Beginning. Meet the Spy Chief Leading Us Into Cyberwar," *Wired*, 12 June 2013. https://www.wired.com/2013/06/general-keith-alexander-cyberwar/ (accessed 2026-11-07).
4. David Kushner, "The Real Story of Stuxnet," *IEEE Spectrum*, 26 February 2013, updated 20 June 2025. https://spectrum.ieee.org/the-real-story-of-stuxnet (accessed 2026-11-07).
5. Nicolas Falliere, Liam O Murchu and Eric Chien, *W32.Stuxnet Dossier, Version 1.4*, Symantec Security Response, February 2011. https://nsarchive2.gwu.edu/NSAEBB/NSAEBB424/docs/Cyber-044.pdf (accessed 2026-11-07).
6. European Union Agency for Cybersecurity (ENISA), "Stuxnet Analysis," n.d. https://www.enisa.europa.eu/news/enisa-news/stuxnet-analysis (accessed 2026-11-07).
7. BBC News, "US and Israel were behind Stuxnet claims researcher," 4 March 2011. https://www.bbc.com/news/technology-12633240 (accessed 2026-11-07).
8. Congressional Research Service, *The Stuxnet Computer Worm: Harbinger of an Emerging Warfare Capability*, R41524, 9 December 2010. https://www.congress.gov/crs_external_products/R/PDF/R41524/R41524.3.pdf (accessed 2026-11-07).
9. David Albright, Paul Brannan and Christina Walrond, "Did Stuxnet Take Out 1,000 Centrifuges at the Natanz Enrichment Plant?," Institute for Science and International Security, 22 December 2010. https://isis-online.org/isis-reports/did-stuxnet-take-out-1000-centrifuges-at-the-natanz-enrichment-plant/ (accessed 2026-11-07).
10. Chenda Ngak, "NSA leaker Snowden claimed U.S. and Israel co-wrote Stuxnet virus," *CBS News*, 9 July 2013. https://www.cbsnews.com/news/nsa-leaker-snowden-claimed-us-and-israel-co-wrote-stuxnet-virus/ (accessed 2026-11-07).

[1]: https://www.nytimes.com/2012/06/01/world/middleeast/obama-ordered-wave-of-cyberattacks-against-iran.html
[2]: https://www.washingtonpost.com/world/national-security/stuxnet-was-work-of-us-and-israeli-experts-officials-say/2012/06/01/gJQAlnEy6U_story.html
[3]: https://www.wired.com/2013/06/general-keith-alexander-cyberwar/
[4]: https://spectrum.ieee.org/the-real-story-of-stuxnet
[5]: https://nsarchive2.gwu.edu/NSAEBB/NSAEBB424/docs/Cyber-044.pdf
[6]: https://www.enisa.europa.eu/news/enisa-news/stuxnet-analysis
[7]: https://www.bbc.com/news/technology-12633240
[8]: https://www.congress.gov/crs_external_products/R/PDF/R41524/R41524.3.pdf
[9]: https://isis-online.org/isis-reports/did-stuxnet-take-out-1000-centrifuges-at-the-natanz-enrichment-plant/
[10]: https://www.cbsnews.com/news/nsa-leaker-snowden-claimed-us-and-israel-co-wrote-stuxnet-virus/
