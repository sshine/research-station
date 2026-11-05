# Source ledger (addendum to the pipeline's `sources.jsonl`)

All URLs retrieved 2026-11-05. The pipeline's `sources.jsonl` covers the 29 sources from
the initial `just research` run; this addendum records the primary/retail sources used
for the final report.

## Shelly official

| URL | Used for |
|---|---|
| https://www.shelly.com/products/shelly-plug-s-gen3 | Discontinued status, recommended successor, S Gen3 specs, MQTT, Matter, local UI |
| https://kb.shelly.cloud/knowledge-base/shelly-plug-s-mtr-gen3 | "S MTR Gen3" naming, model `S3PL-00112EU`, detailed specs |
| https://www.shelly.com/products/shelly-plug-m-gen3-white | M Gen3 specs (13 A / 3000 W, 46 × 46 × 85 mm, MQTT, price 108 DKK) |
| https://www.shelly.com/pages/shelly-smart-plugs | Family comparison, official prices, stock (S and PM sold out) |
| https://support.shelly.cloud/en/support/solutions/articles/103000390376-comparison-of-shelly-plug-s-gen3-shelly-plug-pm-gen3-shelly-plug-m-gen3-shelly-outdoor-plug-s-gen3 | PM Gen3 has no relay; family feature table |
| https://support.shelly.cloud/en/support/solutions/articles/103000406137-support-scope-for-powered-by-shelly-devices | Definition of "Powered by Shelly" third-party program |
| https://us.shelly.com/collections/powered-by-shelly | Examples of Powered by Shelly devices (no plugs in collection) |
| https://matter-smarthome.de/en/products/shelly-announces-additional-matter-products-for-2026 | Plug C Gen4 / Plug C PM Gen4 announced |

## Danish retail

| URL | Used for |
|---|---|
| https://www.proshop.dk/?s=shelly+plug | 9 genuine Shelly results; S/M/PM/Outdoor prices and stock |
| https://www.proshop.dk/?s=shelly+kompatibel | No third-party clone shelf; Shelly accessories |
| https://www.proshop.dk/?s=smart+stikkontakt | 4105 loosely matched results; noise warning |
| https://www.proshop.dk/Smart-Home/Shelly-Plug-S-Gen3-Black/3302035 | S Gen3 black 116 DKK, remote warehouse |
| https://www.proshop.dk/Smart-Home/Shelly-Plug-S-Gen3-White/3302036 | S Gen3 white 137 DKK, remote warehouse |
| https://www.proshop.dk/Smart-Home/Shelly-Plug-PM-Gen3-White-WiFi-16A-smartplug-med-effektmaaler-uden-relae/3488110 | PM Gen3 143 DKK, no relay |
| https://www.proshop.dk/Smart-Home/Shelly-Outdoor-Plug-S-Gen3-WiFi-smart-plug/3351764 | Outdoor 167 DKK |
| https://www.proshop.dk/Smart-Home/Sonoff-S60ZBTPF-ZigBee-smart-stikkontakt/3491818 | Sonoff 120 DKK, 16 A / 4000 W |
| https://www.proshop.dk/Smart-Home/THIRDREALITY-Smart-Plug-E2-Type-F/3397625 | THIRDREALITY 129 DKK, 16 A, out of stock |
| https://www.proshop.dk/Smart-Home/frient-Smart-Plug-Mini-2-Schuko-F/3208952 | frient 268 DKK |
| https://www.proshop.dk/Smart-Home/Eve-Energy-Smart-Plug-Power-Meter-Matter/3154143 | Eve 268 DKK |
| https://www.proshop.dk/Smart-Home/Aqara-Matter-Smart-Power-Plug-H2-EU-RGB/3494479 | Aqara H2 259 DKK |
| https://www.proshop.dk/Smart-Home/Osram-SMART-MATTER-Plug-energimaaler-10A-hvid-WiFi/3409987 | Osram 10 A 90 DKK |
| https://www.proshop.dk/Smart-Home/Osram-SMART-MATTER-Outdoor-Compact-Plug-16A-energimaaler-WiFi/3409988 | Osram outdoor 16 A 122 DKK |
| https://www.proshop.dk/Smart-Home/Meross-Matter-Smart-Plug-with-Enery-Monitor-2-Pack/3399471 | Meross 2-pack 269 DKK |
| https://www.proshop.dk/Smart-Home/Bosch-Smart-stikkontakt-kompakt-M/3418724 | Bosch +M 349 DKK |
| https://www.proshop.dk/Smart-Home/Gosund-EP2-Z-10A-Smart-ZigBee-Outlet/3408582 | Gosund 114 DKK, 10 A |
| https://www.avxperten.dk/shelly-smart-home-stikkontakt | S Gen3 white 115 DKK; EcoFlow "Shelly Kompatibel" 359 DKK |
| https://www.avxperten.dk/ecoflow-stream-smart-plug-shelly-kompatibel-2500w.asp | EcoFlow co-brand listing; brand Shelly + EcoFlow |
| https://www.pricerunner.dk/pl/710-3367353712/Elartikler/Shelly-Plug-S-Gen3-White-Sammenlign-Priser | Price comparison page; S Gen3 still listed by multiple shops |

## Manufacturer documentation for alternatives

| URL | Used for |
|---|---|
| https://www.evehome.com/en/eve-energy | Eve EU 11 A / 2500 W; no Eve cloud; Matter/Thread |
| https://www.aqara.com/en/product/power-plug-h2-eu | Aqara H2 16 A / 3680 W; Matter/Thread + Zigbee; energy monitoring |
| https://www.frient.com/products/smart-plug-mini-2 | frient 10 A continuous / 16 A peak; power meter |
| https://nous.technology/product/a1t2.html | Nous A1T Tasmota, MQTT, 10 A / 2500 W, compact |
| https://www.smarterhomeshop.eu/Nous-A1T-WiFi-smart-plug-with-power-meter-factory | Nous €16.67; 10 A / 2400 W; REST API/MQTT; no cloud |
| https://eu.ecoflow.com/products/shelly-smart-plug | EcoFlow × Shelly 12 A / 2500 W; only STREAM compatible; €34.90 |
| https://www.bosch-smarthome.com/gb/en/support/help/product-help/plug-compact-m-help | Bosch +M Matter/Thread + Zigbee |
| https://matterhome.io/devices/bosch-smart-plug-compact-m | Bosch measures current power and accumulated energy |

## Independent reviews / community caveats

| URL | Used for |
|---|---|
| https://www.cnx-software.com/2025/06/18/sonoff-iplug-s60-zigbee-smart-plug-s60zbtpf-review-with-ewelink-and-home-assistant | Sonoff S60ZBTPF: Z2M exposes more than ZHA |
| https://neilturner.me.uk/2025/09/17/energy-monitoring-over-matter | Matter energy monitoring can stay in vendor app |
| https://smarthomescene.com/reviews/meross-matter-smart-plug-mss315-review-and-home-assistant-integration | Meross MSS315 16 A rating, HA integration |
| https://community.home-assistant.io/t/bosch-smart-plugs-not-reporting-power-consumption-via-z2m/793640 | Bosch power data missing in Z2M |
| https://www.reddit.com/r/homeassistant/comments/1qm0mdr/amazon_sent_me_2_shelly_pm_mini_gen3_that_look | Anecdotal report of lookalike Shelly PM Mini packaging |
