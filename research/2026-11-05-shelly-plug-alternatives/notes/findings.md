# Findings — Shelly Plug S Gen3 and smart-plug alternatives (DK)

Retrieved 2026-11-05. Prices are DKK incl. VAT unless stated. Stock and prices are
snapshots and will change.

## 1. Status of the Shelly Plug S Gen3

- Shelly's own product page says: **"This product is a discontinued product and is no
  longer produced and sold by Shelly Group."** It names the **Shelly Plug M Gen3 White**
  as the "Recommended successor".
  Source: https://www.shelly.com/products/shelly-plug-s-gen3
- Shelly's knowledge base calls the same device **Shelly Plug S MTR Gen3**; device model
  `S3PL-00112EU`. "MTR" is the Matter-capable variant name, not a different product.
  Source: https://kb.shelly.cloud/knowledge-base/shelly-plug-s-mtr-gen3
- Key S Gen3 specs: 12 A / 2500 W resistive, 44 × 44 × 70 mm, Wi-Fi 2.4 GHz + Bluetooth
  4.2, Matter, power/energy metering, multicolor LED, scripting, webhooks, **MQTT: Yes**,
  local web UI, no hub required.
  Source: https://www.shelly.com/products/shelly-plug-s-gen3
- Shelly's family comparison:
  - **Plug S MTR Gen3**: 12 A / 2500 W, Matter, metering, compact.
  - **Plug M Gen3**: 13 A / 3000 W, Matter, metering, 46 × 46 × 85 mm.
  - **Plug PM Gen3**: 16 A / 3680 W, metering only, **no relay**, **no Matter**.
  - **Outdoor Plug S Gen3**: 12 A / 2500 W, IP44, Matter.
  Source: https://www.shelly.com/pages/shelly-smart-plugs
- Shelly support repeats the important PM warning: PM Gen3 is "power measurement
  capabilities only"; the comparison table has "Relay ON/OFF: **No**" for PM Gen3.
  Source: https://support.shelly.cloud/en/support/solutions/articles/103000390376-comparison-of-shelly-plug-s-gen3-shelly-plug-pm-gen3-shelly-plug-m-gen3-shelly-outdoor-plug-s-gen3
- Shelly's own shop on 2026-11-05: S Gen3 199 DKK **sold out**, PM Gen3 199 DKK
  **sold out**, M Gen3 108 DKK in stock, Outdoor 265 DKK in stock.
  Source: https://www.shelly.com/pages/shelly-smart-plugs

## 2. What is still on Danish shelves

### Proshop

Search `https://www.proshop.dk/?s=shelly+plug` returned **9 varer**, all genuine Shelly
products (no third-party lookalikes):

| Product | Price | Stock note |
|---|---:|---|
| Shelly Outdoor Plug S Gen3 | 167 DKK | in stock, next-day |
| Shelly Plug S Gen3 Black | 116 DKK | remote warehouse, 2–3 days; expected own stock 06-11-2026 |
| Shelly Plug S Gen3 White | 137 DKK | remote warehouse, 4–5 days; expected own stock 10-11-2026 |
| Shelly Plug M Gen3 4-pack | 259 DKK | in stock, next-day |
| Shelly Plug M Gen3 2-pack | 137 DKK | in stock, next-day |
| Shelly Plug PM Gen3 White | 143 DKK | in stock, next-day |
| Shelly Plug PM Gen3 Black | 143 DKK | in stock, next-day |
| Shelly Plug M Gen3 4-pack Black | 259 DKK | in stock, next-day |
| Shelly Plus Smoke Alarm | 403 DKK | not a plug |

Source: https://www.proshop.dk/?s=shelly+plug (retrieved 2026-11-05).

So the S Gen3 is still available as residual/remote stock, but it is no longer a
reliable long-term purchase. The "restlager" is not labelled as such; Proshop calls it
"Fjernlager" and gives an expected date for its own stock.

### avXperten

- Shelly Plug S Gen3 White: **115 DKK**, in stock, delivery 1 weekday.
- EcoFlow Stream Smart Plug — **"Shelly Kompatibel, 2500W"**: 359 DKK, in stock.
  Product page lists brand as both **Shelly** and **EcoFlow**; it is an EcoFlow × Shelly
  co-branded plug, not a counterfeit.
  Sources: https://www.avxperten.dk/shelly-smart-home-stikkontakt and
  https://www.avxperten.dk/ecoflow-stream-smart-plug-shelly-kompatibel-2500w.asp
- EcoFlow's own page: "EcoFlow × Shelly Smart Plug", €34.90, 12 A, 44 × 44 × 70 mm,
  2500 W, Wi-Fi + Bluetooth, real-time power monitoring, **only compatible with EcoFlow
  STREAM Series**.
  Source: https://eu.ecoflow.com/products/shelly-smart-plug

## 3. The strongest alternatives

### Same brand / closest replacement

| Product | Why it fits | Price / source |
|---|---|---|
| **Shelly Plug M Gen3** | Official successor to S Gen3. 13 A / 3000 W, Matter, Wi-Fi/BT, power metering, MQTT, local web UI, scripting, 46 × 46 × 85 mm. | 108 DKK single at Shelly; 137 DKK for a 2-pack at Proshop (≈68.50 each). |
| **Shelly Plug PM Gen3** | 16 A / 3680 W metering for always-on loads. **No relay** — cannot switch the load. | 143 DKK at Proshop. |
| **Shelly Outdoor Plug S Gen3** | IP44, 12 A / 2500 W, Matter, metering. | 167 DKK at Proshop. |
| **Shelly Wave Plug S LR** | Z-Wave 800 Long Range version if you already have a Z-Wave hub. | 398 DKK at Shelly. |

Sources: https://www.shelly.com/products/shelly-plug-m-gen3-white,
https://www.proshop.dk/?s=shelly+plug, https://www.shelly.com/pages/shelly-smart-plugs.

### Other brands (shortlist)

| Product | Protocol / ecosystem | Max load | Metering | Local-control note | Hub needed | Price |
|---|---|---:|---|---|---|---:|
| **Eve Energy** (Matter) | Matter over Thread | 11 A / 2500 W (EU) | Yes | Matter local; "No Eve cloud", no registration | Thread border router | 268 DKK |
| **Aqara Power Plug H2 EU (RGB)** | Matter over Thread **or** Zigbee 3.0 | 16 A / 3680 W | Yes | Matter local via controller, or Zigbee via Aqara hub | Thread border router or Aqara hub | 259 DKK |
| **Sonoff S60ZBTPF** | Zigbee 3.0 | 16 A / 4000 W | Yes | Zigbee local; Zigbee2MQTT exposes all entities, ZHA had missing features | Zigbee hub/coordinator | 120 DKK |
| **THIRDREALITY Smart Plug E2** | Zigbee 3.0 | 16 A | Yes | Zigbee local | Zigbee hub | 129 DKK (out of stock) |
| **frient Smart Plug Mini 2** | Zigbee 3.0 | 10 A continuous / 16 A peak | Yes | Zigbee local | Zigbee hub | 268 DKK |
| **Osram SMART+ MATTER Plug** | Matter over Wi-Fi | 10 A / 2300 W | Yes | Matter local | Matter controller | 90 DKK |
| **Osram SMART+ MATTER Outdoor** | Matter over Wi-Fi, IP44 | 16 A / 3680 W | Yes | Matter local | Matter controller | 122 DKK |
| **Meross Matter Smart Plug 2-pack** | Matter (Wi-Fi) | 16 A class | Yes, but often app-only | Matter on/off local; energy may not be exposed to the controller | Matter controller | 269 DKK |
| **Bosch Smart Plug Compact [+M]** | Matter over Thread + Zigbee | 3680 W | Yes | Matter local; Z2M users report no power data | Thread border router / Bosch controller | 349 DKK |
| **Gosund EP2-Z** | Zigbee 3.0 | 10 A | Yes | Zigbee local via hub; Gosund app cloud | Zigbee hub | 114 DKK (single) / 208 DKK (2-pack) |
| **Nous A1T** | Wi-Fi + Tasmota | 10 A / 2400–2500 W | Yes | Local web UI, REST API, MQTT; no factory cloud | No | €16.67 (EU warehouse) |

Sources:
- Eve: https://www.evehome.com/en/eve-energy and
  https://www.proshop.dk/Smart-Home/Eve-Energy-Smart-Plug-Power-Meter-Matter/3154143
- Aqara: https://www.aqara.com/en/product/power-plug-h2-eu and
  https://www.proshop.dk/Smart-Home/Aqara-Matter-Smart-Power-Plug-H2-EU-RGB/3494479
- Sonoff: https://www.proshop.dk/Smart-Home/Sonoff-S60ZBTPF-ZigBee-smart-stikkontakt/3491818
  and https://www.cnx-software.com/2025/06/18/sonoff-iplug-s60-zigbee-smart-plug-s60zbtpf-review-with-ewelink-and-home-assistant
- THIRDREALITY: https://www.proshop.dk/Smart-Home/THIRDREALITY-Smart-Plug-E2-Type-F/3397625
- frient: https://www.frient.com/products/smart-plug-mini-2 and
  https://www.proshop.dk/Smart-Home/frient-Smart-Plug-Mini-2-Schuko-F/3208952
- Osram 10 A: https://www.proshop.dk/Smart-Home/Osram-SMART-MATTER-Plug-energimaaler-10A-hvid-WiFi/3409987
- Osram outdoor: https://www.proshop.dk/Smart-Home/Osram-SMART-MATTER-Outdoor-Compact-Plug-16A-energimaaler-WiFi/3409988
- Meross: https://www.proshop.dk/Smart-Home/Meross-Matter-Smart-Plug-with-Enery-Monitor-2-Pack/3399471
  and https://neilturner.me.uk/2025/09/17/energy-monitoring-over-matter
- Bosch: https://www.proshop.dk/Smart-Home/Bosch-Smart-stikkontakt-kompakt-M/3418724,
  https://www.bosch-smarthome.com/gb/en/support/help/product-help/plug-compact-m-help,
  https://community.home-assistant.io/t/bosch-smart-plugs-not-reporting-power-consumption-via-z2m/793640
- Gosund: https://www.proshop.dk/Smart-Home/Gosund-EP2-Z-10A-Smart-ZigBee-Outlet/3408582
- Nous: https://nous.technology/product/a1t2.html and
  https://www.smarterhomeshop.eu/Nous-A1T-WiFi-smart-plug-with-power-meter-factory

## 4. "Shelly" as a keyword: what is actually going on

- **Shelly is still a brand, not a generic word.** A search for `shelly plug` at Proshop
  returned only 9 genuine Shelly items. A search for `Shelly kompatibel` at Proshop
  returned Shelly accessories, not a third-party clone shelf.
  Sources: https://www.proshop.dk/?s=shelly+plug and
  https://www.proshop.dk/?s=shelly+kompatibel
- **But "Shelly" is used as a compatibility/category label.** avXperten sells an
  "EcoFlow Stream Smart Plug - **Shelly Kompatibel**, 2500W" for 359 DKK. The page lists
  brand as both Shelly and EcoFlow; EcoFlow sells it as an "EcoFlow × Shelly Smart Plug".
  This is a co-branded product, not a copycat. It is also limited to EcoFlow STREAM.
  Sources: https://www.avxperten.dk/ecoflow-stream-smart-plug-shelly-kompatibel-2500w.asp
  and https://eu.ecoflow.com/products/shelly-smart-plug
- **Shelly has an official third-party program, "Powered by Shelly".** Partners integrate
  a Shelly connectivity module and are compatible with the Shelly app. Shelly lists
  Ogemray, Ecowitt, FrankEver, Cury and others. The current "Powered by Shelly" shop
  collection had no smart plugs — it was relays, weather stations and irrigation
  controllers.
  Sources: https://support.shelly.cloud/en/support/solutions/articles/103000406137-support-scope-for-powered-by-shelly-devices
  and https://us.shelly.com/collections/powered-by-shelly
- **Independent brands are not copies.** Sonoff, Aqara, Eve, Meross, Osram, Bosch, Nous
  and Athom have their own apps/ecosystems and different local-control trade-offs. They
  are "Shelly alternatives" in the feature sense, not counterfeit Shellys.
- **Counterfeits do exist on marketplaces**, so buy from a reputable retailer and check
  the model/box. I found no evidence of a widespread counterfeit Shelly-plug shelf in
  Danish retail; the more likely explanation for "a bunch of alternatives" is a broader
  search query (`smart plug`, `smart stikkontakt`) or compatibility wording.
  Anecdotal example: https://www.reddit.com/r/homeassistant/comments/1qm0mdr/amazon_sent_me_2_shelly_pm_mini_gen3_that_look

## 5. Caveats / uncertainty

- Prices, stock and even product pages change daily. The Proshop search page is dynamic
  and may return different counts on another day.
- "Local control" is not binary. A plug can pair locally over Matter/Zigbee while its
  energy history stays in the vendor app. In particular, several Matter plugs expose
  on/off but not power/energy to third-party controllers.
- Zigbee energy reporting quality depends on the coordinator: Zigbee2MQTT exposed more
  entities than ZHA for the Sonoff S60ZBTPF in the cited review.
- The Shelly Plug S Gen3's "restlager" stock is remote warehouse, not necessarily on the
  shelf; the expected own-stock dates were 06-11 and 10-11-2026.
- The 2026 Shelly catalogue also shows a new **Plug C Gen4 / Plug C PM Gen4** cord-end
  design, but it was not broadly listed in Danish retail at the time of this check.
  Source: https://matter-smarthome.de/en/products/shelly-announces-additional-matter-products-for-2026
