# Smart plugs that do what the Shelly Plug S Gen3 did

*Research snapshot: 5 November 2026. Prices are DKK including VAT; stock and prices
change. This is a shortlist of the strongest options, not an exhaustive catalogue.*

## Bottom line

1. **The Shelly Plug S Gen3 is officially discontinued.** Shelly's own product page says
   it is "no longer produced and sold by Shelly Group" and names the **Shelly Plug M
   Gen3** as the recommended successor. The M Gen3 is the default buy: 13 A / 3000 W,
   Matter, Wi-Fi/Bluetooth, power metering, MQTT and a local web UI — and it is cheaper
   than the old S Gen3. Shelly S Gen3 [1]
   · Shelly M Gen3 [2]
2. **You can still get the S Gen3 as residual stock.** Proshop had the black at 116 DKK
   and the white at 137 DKK on "fjernlager" (remote warehouse, 2–5 days); avXperten had
   the white at 115 DKK in stock. Treat this as end-of-line stock, not a long-term
   supply. Proshop search [3] ·
   avXperten [4]
3. **If you want another brand, the best all-round alternative is the Aqara Power Plug H2
   EU** (259 DKK, 16 A / 3680 W, Matter over Thread or Zigbee, energy monitoring). For
   privacy/no cloud, choose **Eve Energy** (268 DKK, Matter/Thread, no Eve cloud). For a
   cheap Zigbee plug, choose **Sonoff S60ZBTPF** (120 DKK, 16 A, needs a hub). For budget
   Matter, **Osram SMART+ Matter 10 A** is 90 DKK if 10 A is enough.
4. **"Shelly" has not become a generic word for smart plugs.** A Proshop search for
   `shelly plug` returned **9 varer**, all genuine Shelly. The alternatives appear when
   you search the category (`smart stikkontakt`, `smart plug`). "Shelly kompatibel" is
   used as a compatibility label, and at least one such product is an official
   EcoFlow × Shelly co-brand, not a clone.

## 1. What the Shelly Plug S Gen3 actually is

Shelly's knowledge base also calls it the **Shelly Plug S MTR Gen3** (model
`S3PL-00112EU`); "MTR" is the Matter-capable variant name, not a separate product.
Shelly KB [5]

Key specs from Shelly's product page: **12 A / 2500 W** resistive load, 44 × 44 × 70 mm,
Wi-Fi 2.4 GHz + Bluetooth, **Matter**, power/energy metering, multicolor LED, scripting,
webhooks, **MQTT**, local web interface, no hub required.
Shelly S Gen3 [1]

The Gen3 plug family, according to Shelly's comparison page:

| Model | Status | Max load | Relay | Metering | Matter | Notes |
|---|---|---:|---|---|---|---|
| **Plug S Gen3 / S MTR Gen3** | discontinued | 12 A / 2500 W | yes | yes | yes | 44 × 44 × 70 mm, most compact |
| **Plug M Gen3** | current | 13 A / 3000 W | yes | yes | yes | 46 × 46 × 85 mm, successor |
| **Plug PM Gen3** | current | 16 A / 3680 W | **no** | yes | **no** | monitoring only; cannot switch |
| **Outdoor Plug S Gen3** | current | 12 A / 2500 W | yes | yes | yes | IP44 |

Source: Shelly smart plugs comparison [6];
the PM's missing relay is also explicit in Shelly's support comparison
(support article [7]).

Shelly's own shop on 5 November 2026 showed the S Gen3 and PM Gen3 **sold out**, while
the M Gen3 (108 DKK) and Outdoor (265 DKK) were in stock. That is consistent with the
S Gen3 being phased out and the M Gen3 being the replacement.

## 2. Can you still buy the S Gen3 in Denmark?

Yes, but only as residual/remote stock. Proshop's `shelly plug` search returned 9 items,
all genuine Shelly:

| Product | Price | Stock note |
|---|---:|---|
| Shelly Outdoor Plug S Gen3 | 167 DKK | in stock, next-day |
| Shelly Plug S Gen3 Black | 116 DKK | remote warehouse, 2–3 days; expected own stock 06-11-2026 |
| Shelly Plug S Gen3 White | 137 DKK | remote warehouse, 4–5 days; expected own stock 10-11-2026 |
| Shelly Plug M Gen3 2-pack | 137 DKK | in stock, next-day |
| Shelly Plug M Gen3 4-pack | 259 DKK | in stock, next-day |
| Shelly Plug PM Gen3 (white/black) | 143 DKK | in stock, next-day |

Source: Proshop `?s=shelly+plug` [3], retrieved
2026-11-05. Proshop does not label these "restlager"; it uses "Fjernlager" and gives an
expected date for its own stock.

avXperten listed the white S Gen3 at **115 DKK**, in stock with one-day delivery, and the
black at a higher price. avXperten Shelly category [4]

## 3. Alternatives worth buying

### Same brand (simplest path)

- **Shelly Plug M Gen3** — the official successor. 13 A / 3000 W, Matter, Wi-Fi/BT,
  metering, MQTT, local web UI, scripting. 108 DKK single from Shelly; **137 DKK for a
  2-pack at Proshop** (about 69 DKK each), 259 DKK for a 4-pack. It is 15 mm taller than
  the S Gen3 but otherwise the same idea, with a higher load rating.
  Shelly M Gen3 [2] ·
  Proshop search [3]
- **Shelly Plug PM Gen3** — only if you want to *measure* an always-on appliance and
  never switch it. 16 A / 3680 W, **no relay**, no Matter. 143 DKK at Proshop.
  Shelly comparison [6]
- **Shelly Outdoor Plug S Gen3** — IP44, 12 A / 2500 W, Matter, metering. 167 DKK at
  Proshop.
- **Shelly Wave Plug S LR** — Z-Wave 800 Long Range version if you already have a Z-Wave
  hub. 398 DKK from Shelly. Shelly smart plugs [6]

### Other brands

| Product | Protocol | Max load | Metering | Local control / cloud | Hub needed | Price |
|---|---|---:|---|---|---|---:|
| **Eve Energy** | Matter over Thread | 11 A / 2500 W (EU) | yes | Matter local; no Eve cloud, no registration | Thread border router | 268 DKK |
| **Aqara Power Plug H2 EU (RGB)** | Matter over Thread or Zigbee 3.0 | 16 A / 3680 W | yes | Matter local, or Zigbee via Aqara hub | Thread router or Aqara hub | 259 DKK |
| **Sonoff S60ZBTPF** | Zigbee 3.0 | 16 A / 4000 W | yes | Zigbee local; Zigbee2MQTT exposes the most entities | Zigbee hub/coordinator | 120 DKK |
| **THIRDREALITY Smart Plug E2** | Zigbee 3.0 | 16 A | yes | Zigbee local | Zigbee hub | 129 DKK (out of stock) |
| **frient Smart Plug Mini 2** | Zigbee 3.0 | 10 A continuous / 16 A peak | yes | Zigbee local | Zigbee hub | 268 DKK |
| **Osram SMART+ MATTER Plug** | Matter over Wi-Fi | 10 A / 2300 W | yes | Matter local | Matter controller | 90 DKK |
| **Osram SMART+ MATTER Outdoor** | Matter over Wi-Fi, IP44 | 16 A / 3680 W | yes | Matter local | Matter controller | 122 DKK |
| **Meross Matter Smart Plug 2-pack** | Matter (Wi-Fi) | 16 A | yes, often app-only | Matter on/off local; energy may not reach the controller | Matter controller | 269 DKK |
| **Bosch Smart Plug Compact [+M]** | Matter over Thread + Zigbee | 3680 W | yes | Matter local; Z2M power reporting is unreliable | Thread router / Bosch controller | 349 DKK |
| **Gosund EP2-Z** | Zigbee 3.0 | 10 A | yes | Zigbee local via hub; Gosund app is cloud | Zigbee hub | 114 DKK single / 208 DKK 2-pack |
| **Nous A1T** | Wi-Fi + Tasmota | 10 A / 2400–2500 W | yes | Local web UI, REST API, MQTT; no factory cloud | No | €16.67 (EU warehouse) |

Prices from Proshop on 2026-11-05 unless noted: Eve [8],
Aqara [9],
Sonoff [10],
THIRDREALITY [11],
frient [12],
Osram 10 A [13],
Osram outdoor [14],
Meross [15],
Bosch [16],
Gosund [17].

Specs and caveats:

- **Eve Energy** is the privacy option: Matter over Thread, no Eve cloud, no
  registration. EU version is 11 A / 2500 W, so it is a lamp/office plug, not a heater
  plug. A Thread border router is required. Eve [18]
- **Aqara H2 EU** is the strongest all-round non-Shelly choice: 16 A / 3680 W, energy
  monitoring, Matter over Thread *or* Zigbee 3.0. You need a Thread border router or an
  Aqara hub; it is not a Wi-Fi plug. Aqara [19]
- **Sonoff S60ZBTPF** is the cheap high-load Zigbee option: 16 A / 4000 W, Zigbee 3.0,
  metering, repeater. It needs a Zigbee hub. Zigbee2MQTT exposes all entities; ZHA has
  had missing features in reviews, so check the current state before buying.
  CNX review [20]
- **THIRDREALITY E2** is also 16 A Zigbee with real-time metering, but it was out of
  stock at Proshop on the check date.
- **frient Mini 2** is well built but only 10 A continuous; it is not for a 16 A heater.
  frient [21]
- **Osram SMART+ Matter** is the budget Matter route: 90 DKK for 10 A / 2300 W, 122 DKK
  for the 16 A / 3680 W IP44 outdoor version. Both are Matter over Wi-Fi, so no hub, but
  you need a Matter controller.
- **Meross Matter** is a 16 A plug and cheap for a 2-pack, but its energy data is primarily
  in the Meross app. Several users report that Matter controllers get on/off but not
  power/energy. MSS315 specs [22] ·
  Matter energy-monitoring caveat [23]
- **Bosch [+M]** supports Matter/Thread and Zigbee and measures power, but it is expensive
  and Zigbee2MQTT users report missing power data. Buy it for the Bosch ecosystem, not
  as a cheap Shelly replacement.
  Bosch help [24] ·
  power measurement [25]
- **Gosund EP2-Z** is a cheap 10 A Zigbee plug; fine for lamps and small appliances, not
  for high-load heating.
- **Nous A1T** is the closest thing to a "Shelly for tinkerers": factory-installed
  Tasmota, MQTT, REST API, local web UI, no cloud, 10 A / 2400–2500 W, compact 46 × 46 ×
  72 mm. It is not sold at Proshop; the EU warehouse price was €16.67.
  Nous [26] ·
  EU shop [27]

## 4. Is "Shelly" becoming a keyword for copy-cats?

Partly, but not in the way it first looks.

- **Shelly is still a brand.** Proshop's `shelly plug` search returned 9 genuine Shelly
  products and no third-party lookalikes. A `shelly kompatibel` search returned Shelly
  accessories, not a clone shelf. Proshop `shelly plug` [3] ·
  Proshop `shelly kompatibel` [28]
- **"Shelly kompatibel" is a compatibility label.** avXperten sells an "EcoFlow Stream
  Smart Plug — Shelly Kompatibel, 2500W" for 359 DKK. The product page lists brand as both
  Shelly and EcoFlow. EcoFlow itself calls it the "EcoFlow × Shelly Smart Plug" (€34.90,
  12 A, 44 × 44 × 70 mm, 2500 W) and says it is only compatible with the EcoFlow STREAM
  system. This is a co-brand, not a counterfeit. avXperten [29] ·
  EcoFlow [30]
- **Shelly has an official third-party program.** "Powered by Shelly" devices contain a
  Shelly connectivity module and work with the Shelly app; the product itself is made by
  the partner. Shelly's current examples are the Ogemray 25 A relay, Ecowitt WS90 weather
  station, FrankEver sprinkler controller and similar devices — no smart plugs in that
  collection on the check date. Program definition [31] ·
  Collection [32]
- **Independent brands are not copies.** Sonoff, Aqara, Eve, Meross, Osram, Bosch, Nous
  and Athom have their own apps, protocols and trade-offs. Reviewers call them "Shelly
  alternatives" because they match the feature set, not because they imitate the brand.
- **Counterfeits exist on marketplaces**, so buy from a reputable retailer and check the
  model/box. There are public reports of lookalike Shelly PM Mini boxes on Amazon
  (example [33]),
  but I found no evidence of a widespread counterfeit Shelly-plug shelf in Danish retail.
  The more likely explanation for "a bunch of alternatives" is a broader search query or
  compatibility wording.

## 5. The keywords that actually find this category

| What you want | Danish search terms | English terms |
|---|---|---|
| Generic product | smart stikkontakt, smartstik, smart plug | smart plug, smart socket |
| Power measurement | stikkontakt med energimåling, stikkontakt med effektmåler, energimåler | smart plug with energy monitoring, power monitoring plug |
| Remote/timer | fjernbetjent stikkontakt, programmerbar stikkontakt, stikkontakt med timer | remote control plug, programmable plug |
| Wi-Fi | WiFi-stikkontakt, trådløs stikkontakt | Wi-Fi smart plug |
| Zigbee | Zigbee-stikkontakt, Zigbee smartstik | Zigbee smart plug |
| Matter/Thread | Matter-stikkontakt, Thread-stikkontakt | Matter smart plug, Thread smart plug |
| Z-Wave | Z-Wave-stikkontakt | Z-Wave smart plug |
| Outdoor | udendørs smartstikkontakt, stikkontakt IP44 | outdoor smart plug, IP44 smart plug |
| High load | 16A stikkontakt, 3680W stikkontakt | 16A smart plug, 3680W smart plug |
| Local-first | lokal styring, lokal API, ingen cloud, MQTT, Tasmota, ESPHome, Home Assistant | local control, no cloud, MQTT, Tasmota, ESPHome, Home Assistant |

A practical search ladder:

1. `smart stikkontakt energimåling` — broad Danish retail.
2. `stikkontakt effektmåler Matter` — modern ecosystem plugs.
3. `Zigbee stikkontakt energimåling` — hub-based plugs.
4. `Tasmota stikkontakt` or `ESPHome smart plug` — local-first, no cloud.
5. `Shelly kompatibel stikkontakt` — compatibility/co-brand results, not a brand list.
6. Add `site:proshop.dk`, `site:avxperten.dk` or `site:pricerunner.dk` to compare Danish
   stock and prices.

One warning: `smart stikkontakt` on Proshop returned **4105 varer**, because the search
matches loosely and pulls in cameras and other Smart Home gear. Add `energimåling`,
`effektmåler`, `Zigbee`, `Matter` or `16A` to narrow it. The raw count is not a product
count. Proshop `smart stikkontakt` [34]

## 6. What I would buy

- **Simplest, best value:** Shelly Plug M Gen3 2-pack, 137 DKK at Proshop. Same local
  API/MQTT/Matter approach as the S Gen3, higher 13 A rating, and cheaper per plug. If you
  only need one, 108 DKK from Shelly.
- **Exact S Gen3:** avXperten white 115 DKK (in stock) or Proshop black 116 DKK (remote
  warehouse). Do not pay much more; it is discontinued.
- **No cloud / privacy:** Eve Energy (268 DKK) if 11 A is enough and you have a Thread
  border router; Nous A1T (€16.67) if you are comfortable with Tasmota and MQTT.
- **Zigbee hub already installed:** Sonoff S60ZBTPF (120 DKK, 16 A) for value, or Aqara H2
  EU (259 DKK, 16 A, Matter/Thread + Zigbee) for a more polished dual-protocol plug.
- **Cheap Matter:** Osram SMART+ Matter 10 A (90 DKK) for lamps/small appliances; the
  16 A outdoor version is 122 DKK.
- **Outdoor:** Shelly Outdoor Plug S Gen3 (167 DKK) if you want the Shelly ecosystem;
  Osram SMART+ Matter Outdoor (122 DKK) if you want cheap Matter/IP44.
- **Measure only, never switch:** Shelly Plug PM Gen3 (143 DKK). Do not buy this expecting
  on/off control.

## 7. Caveats

- Prices and stock are a 5 November 2026 snapshot. Proshop's search results and expected
  stock dates change; "fjernlager" is not the same as "on the shelf".
- "Local control" is not binary. A plug can pair locally over Matter or Zigbee while its
  energy history stays in the vendor app. Check that the controller you use exposes the
  metering cluster, not just on/off.
- Zigbee energy reporting depends on the coordinator. The Sonoff review found
  Zigbee2MQTT exposed more entities than ZHA; check current firmware/integration before
  relying on it.
- Matter energy reporting is still uneven across brands. Eve exposes power/energy through
  Matter; Meross has been app-only in practice
  (Matter energy-monitoring caveat [23]).
  Do not assume that a "Matter" label means your controller will see the meter.
- Shelly's 2026 catalogue also shows a new **Plug C Gen4 / Plug C PM Gen4** cord-end
  design, but it was not broadly listed in Danish retail at the time of this check. If you
  can wait, it may be the more space-saving Gen4 option.
  Matter-SmartHome [35]
- I did not audit every Danish webshop or marketplace. The shortlist is based on Shelly's
  official pages, Proshop/avXperten listings, and manufacturer documentation retrieved on
  the check date.

## References

1. Shelly. *Shelly Plug S Gen3 White | Smart Wi-Fi Plug with Power Monitoring | Shelly Europe*. <https://www.shelly.com/products/shelly-plug-s-gen3> (accessed 5 November 2026).
2. Shelly. *Shelly Plug M Gen3 White*. <https://www.shelly.com/products/shelly-plug-m-gen3-white> (accessed 5 November 2026).
3. Proshop. *Search results for “shelly plug”*. <https://www.proshop.dk/?s=shelly+plug> (accessed 5 November 2026).
4. avXperten. *Shelly smart home stikkontakt*. <https://www.avxperten.dk/shelly-smart-home-stikkontakt> (accessed 5 November 2026).
5. Shelly Knowledge Base. *Shelly Plug S MTR Gen3 | Shelly Knowledge Base*. <https://kb.shelly.cloud/knowledge-base/shelly-plug-s-mtr-gen3> (accessed 5 November 2026).
6. Shelly. *Shelly Smart Plugs | Shelly Europe*. <https://www.shelly.com/pages/shelly-smart-plugs> (accessed 5 November 2026).
7. Shelly Support. *Comparison of Shelly Plug S Gen3, Shelly Plug PM Gen3, Shelly Plug M Gen3, Shelly Outdoor Plug S Gen3 :*. <https://support.shelly.cloud/en/support/solutions/articles/103000390376-comparison-of-shelly-plug-s-gen3-shelly-plug-pm-gen3-shelly-plug-m-gen3-shelly-outdoor-plug-s-gen3> (accessed 5 November 2026).
8. Proshop. *Eve Energy Smart Plug Power Meter Matter*. <https://www.proshop.dk/Smart-Home/Eve-Energy-Smart-Plug-Power-Meter-Matter/3154143> (accessed 5 November 2026).
9. Proshop. *Aqara Matter Smart Power Plug H2 EU RGB*. <https://www.proshop.dk/Smart-Home/Aqara-Matter-Smart-Power-Plug-H2-EU-RGB/3494479> (accessed 5 November 2026).
10. Proshop. *Sonoff S60ZBTPF ZigBee smart stikkontakt*. <https://www.proshop.dk/Smart-Home/Sonoff-S60ZBTPF-ZigBee-smart-stikkontakt/3491818> (accessed 5 November 2026).
11. Proshop. *THIRDREALITY Smart Plug E2 Type F*. <https://www.proshop.dk/Smart-Home/THIRDREALITY-Smart-Plug-E2-Type-F/3397625> (accessed 5 November 2026).
12. Proshop. *frient Smart Plug Mini 2 Schuko F*. <https://www.proshop.dk/Smart-Home/frient-Smart-Plug-Mini-2-Schuko-F/3208952> (accessed 5 November 2026).
13. Proshop. *Osram SMART+ MATTER Plug energimåler 10 A hvid WiFi*. <https://www.proshop.dk/Smart-Home/Osram-SMART-MATTER-Plug-energimaaler-10A-hvid-WiFi/3409987> (accessed 5 November 2026).
14. Proshop. *Osram SMART+ MATTER Outdoor Compact Plug 16 A energimåler WiFi*. <https://www.proshop.dk/Smart-Home/Osram-SMART-MATTER-Outdoor-Compact-Plug-16A-energimaaler-WiFi/3409988> (accessed 5 November 2026).
15. Proshop. *Meross Matter Smart Plug with Energy Monitor 2-pack*. <https://www.proshop.dk/Smart-Home/Meross-Matter-Smart-Plug-with-Enery-Monitor-2-Pack/3399471> (accessed 5 November 2026).
16. Proshop. *Bosch Smart stikkontakt kompakt M*. <https://www.proshop.dk/Smart-Home/Bosch-Smart-stikkontakt-kompakt-M/3418724> (accessed 5 November 2026).
17. Proshop. *Gosund EP2-Z 10 A Smart ZigBee Outlet*. <https://www.proshop.dk/Smart-Home/Gosund-EP2-Z-10A-Smart-ZigBee-Outlet/3408582> (accessed 5 November 2026).
18. Eve. *Eve Energy*. <https://www.evehome.com/en/eve-energy> (accessed 5 November 2026).
19. Aqara. *Aqara Power Plug H2 EU*. <https://www.aqara.com/en/product/power-plug-h2-eu> (accessed 5 November 2026).
20. CNX Software. *Sonoff S60ZBTPF review with eWeLink and Home Assistant*. <https://www.cnx-software.com/2025/06/18/sonoff-iplug-s60-zigbee-smart-plug-s60zbtpf-review-with-ewelink-and-home-assistant> (accessed 5 November 2026).
21. frient. *frient Smart Plug Mini 2*. <https://www.frient.com/products/smart-plug-mini-2> (accessed 5 November 2026).
22. SmartHomeScene. *Meross Matter Smart Plug MSS315 review and Home Assistant integration*. <https://smarthomescene.com/reviews/meross-matter-smart-plug-mss315-review-and-home-assistant-integration> (accessed 5 November 2026).
23. Neil Turner. *Energy monitoring over Matter*. <https://neilturner.me.uk/2025/09/17/energy-monitoring-over-matter> (accessed 5 November 2026).
24. Bosch Smart Home. *Plug Compact M help*. <https://www.bosch-smarthome.com/gb/en/support/help/product-help/plug-compact-m-help> (accessed 5 November 2026).
25. MatterHome. *Bosch Smart Plug Compact M*. <https://matterhome.io/devices/bosch-smart-plug-compact-m> (accessed 5 November 2026).
26. Nous. *Nous A1T*. <https://nous.technology/product/a1t2.html> (accessed 5 November 2026).
27. Smarter Home Shop. *Nous A1T Wi-Fi smart plug with power meter — EU shop*. <https://www.smarterhomeshop.eu/Nous-A1T-WiFi-smart-plug-with-power-meter-factory> (accessed 5 November 2026).
28. Proshop. *Search results for “shelly kompatibel”*. <https://www.proshop.dk/?s=shelly+kompatibel> (accessed 5 November 2026).
29. avXperten. *EcoFlow Stream Smart Plug — Shelly Kompatibel, 2500W*. <https://www.avxperten.dk/ecoflow-stream-smart-plug-shelly-kompatibel-2500w.asp> (accessed 5 November 2026).
30. EcoFlow. *EcoFlow × Shelly Smart Plug*. <https://eu.ecoflow.com/products/shelly-smart-plug> (accessed 5 November 2026).
31. Shelly Support. *Support scope for Powered by Shelly devices*. <https://support.shelly.cloud/en/support/solutions/articles/103000406137-support-scope-for-powered-by-shelly-devices> (accessed 5 November 2026).
32. Shelly. *Powered by Shelly collection*. <https://us.shelly.com/collections/powered-by-shelly> (accessed 5 November 2026).
33. Reddit r/homeassistant. *Amazon sent me 2 Shelly PM Mini Gen3 that look like counterfeits*. <https://www.reddit.com/r/homeassistant/comments/1qm0mdr/amazon_sent_me_2_shelly_pm_mini_gen3_that_look> (accessed 5 November 2026).
34. Proshop. *Search results for “smart stikkontakt”*. <https://www.proshop.dk/?s=smart+stikkontakt> (accessed 5 November 2026).
35. Matter-SmartHome. *Shelly announces additional Matter products for 2026*. <https://matter-smarthome.de/en/products/shelly-announces-additional-matter-products-for-2026> (accessed 5 November 2026).

[1]: https://www.shelly.com/products/shelly-plug-s-gen3
[2]: https://www.shelly.com/products/shelly-plug-m-gen3-white
[3]: https://www.proshop.dk/?s=shelly+plug
[4]: https://www.avxperten.dk/shelly-smart-home-stikkontakt
[5]: https://kb.shelly.cloud/knowledge-base/shelly-plug-s-mtr-gen3
[6]: https://www.shelly.com/pages/shelly-smart-plugs
[7]: https://support.shelly.cloud/en/support/solutions/articles/103000390376-comparison-of-shelly-plug-s-gen3-shelly-plug-pm-gen3-shelly-plug-m-gen3-shelly-outdoor-plug-s-gen3
[8]: https://www.proshop.dk/Smart-Home/Eve-Energy-Smart-Plug-Power-Meter-Matter/3154143
[9]: https://www.proshop.dk/Smart-Home/Aqara-Matter-Smart-Power-Plug-H2-EU-RGB/3494479
[10]: https://www.proshop.dk/Smart-Home/Sonoff-S60ZBTPF-ZigBee-smart-stikkontakt/3491818
[11]: https://www.proshop.dk/Smart-Home/THIRDREALITY-Smart-Plug-E2-Type-F/3397625
[12]: https://www.proshop.dk/Smart-Home/frient-Smart-Plug-Mini-2-Schuko-F/3208952
[13]: https://www.proshop.dk/Smart-Home/Osram-SMART-MATTER-Plug-energimaaler-10A-hvid-WiFi/3409987
[14]: https://www.proshop.dk/Smart-Home/Osram-SMART-MATTER-Outdoor-Compact-Plug-16A-energimaaler-WiFi/3409988
[15]: https://www.proshop.dk/Smart-Home/Meross-Matter-Smart-Plug-with-Enery-Monitor-2-Pack/3399471
[16]: https://www.proshop.dk/Smart-Home/Bosch-Smart-stikkontakt-kompakt-M/3418724
[17]: https://www.proshop.dk/Smart-Home/Gosund-EP2-Z-10A-Smart-ZigBee-Outlet/3408582
[18]: https://www.evehome.com/en/eve-energy
[19]: https://www.aqara.com/en/product/power-plug-h2-eu
[20]: https://www.cnx-software.com/2025/06/18/sonoff-iplug-s60-zigbee-smart-plug-s60zbtpf-review-with-ewelink-and-home-assistant
[21]: https://www.frient.com/products/smart-plug-mini-2
[22]: https://smarthomescene.com/reviews/meross-matter-smart-plug-mss315-review-and-home-assistant-integration
[23]: https://neilturner.me.uk/2025/09/17/energy-monitoring-over-matter
[24]: https://www.bosch-smarthome.com/gb/en/support/help/product-help/plug-compact-m-help
[25]: https://matterhome.io/devices/bosch-smart-plug-compact-m
[26]: https://nous.technology/product/a1t2.html
[27]: https://www.smarterhomeshop.eu/Nous-A1T-WiFi-smart-plug-with-power-meter-factory
[28]: https://www.proshop.dk/?s=shelly+kompatibel
[29]: https://www.avxperten.dk/ecoflow-stream-smart-plug-shelly-kompatibel-2500w.asp
[30]: https://eu.ecoflow.com/products/shelly-smart-plug
[31]: https://support.shelly.cloud/en/support/solutions/articles/103000406137-support-scope-for-powered-by-shelly-devices
[32]: https://us.shelly.com/collections/powered-by-shelly
[33]: https://www.reddit.com/r/homeassistant/comments/1qm0mdr/amazon_sent_me_2_shelly_pm_mini_gen3_that_look
[34]: https://www.proshop.dk/?s=smart+stikkontakt
[35]: https://matter-smarthome.de/en/products/shelly-announces-additional-matter-products-for-2026
