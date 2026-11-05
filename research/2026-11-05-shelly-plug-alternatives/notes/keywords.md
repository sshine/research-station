# Search vocabulary and "is Shelly generic?" notes

Retrieved 2026-11-05.

## The short answer

Shelly has **not** become a generic word for "smart plug" in Danish retail. It is still a
brand. What has happened is:

1. Shelly is a high-intent search term, so retailers put it in category names and in
   compatibility text ("Shelly kompatibel", "works with Shelly").
2. Some products are genuine co-brands (e.g. EcoFlow × Shelly Smart Plug), not clones.
3. Shelly runs an official "Powered by Shelly" partner program for third-party devices
   that contain a Shelly connectivity module.
4. The actual generic product words are *smart plug* / *smart stikkontakt* /
   *stikkontakt med energimåling*.

## Danish retail keywords

| Goal | Danish search terms | English equivalents |
|---|---|---|
| Generic product | smart stikkontakt, smartstik, smart plug, smart stik | smart plug, smart socket, smart outlet |
| Power measurement | stikkontakt med energimåling, stikkontakt med effektmåler, energimåler, strømforbrugsmåling | smart plug with energy monitoring, power monitoring plug, energy meter plug |
| Remote / timer | fjernbetjent stikkontakt, programmerbar stikkontakt, stikkontakt med timer | remote control plug, programmable plug, timer plug |
| Wi-Fi | WiFi-stikkontakt, trådløs stikkontakt | Wi-Fi smart plug |
| Zigbee | Zigbee-stikkontakt, Zigbee smartstik | Zigbee smart plug |
| Matter / Thread | Matter-stikkontakt, Thread-stikkontakt | Matter smart plug, Thread smart plug |
| Z-Wave | Z-Wave-stikkontakt | Z-Wave smart plug |
| Outdoor | udendørs smartstikkontakt, stikkontakt IP44 | outdoor smart plug, IP44 smart plug |
| High load | 16A stikkontakt, 3680W stikkontakt | 16A smart plug, 3680W smart plug |
| Compact | mini smartstik, kompakt stikkontakt | mini smart plug, compact smart plug |

## Local-first / power-user keywords

These are the words that separate a Shelly-like plug from a cloud-only Tuya plug:

- **lokal styring**, **lokal API**, **ingen cloud**, **skyfri**
- **MQTT**, **REST API**, **webhooks**, **HTTP API**
- **Home Assistant**, **openHAB**, **Homey**, **Domoticz**
- **Tasmota**, **ESPHome**, **ESP32**, **ESP8266**
- **Matter over Thread**, **Matter over Wi-Fi**, **Zigbee2MQTT (Z2M)**, **ZHA**
- **"no cloud"**, **"local control"**, **"works offline"**

## Compatibility keywords (handle with care)

- **"Shelly kompatibel"** — can mean "works with Shelly", "works alongside Shelly in Home
  Assistant", or simply "same type of plug". It is not proof of a clone.
- **"Works with Shelly"** — Shelly's compatibility program/marketing phrase.
- **"Powered by Shelly"** — official: third-party product contains a Shelly connectivity
  module and works with the Shelly app. Examples: Ogemray 25A relay, Ecowitt WS90 weather
  station, FrankEver sprinkler controller. No smart plugs in that collection on
  2026-11-05.
- **"Shelly alternativ"** — mostly used by reviewers and comparison sites, not a product
  category.

## What the evidence shows

- Proshop `?s=shelly+plug` → **9 varer**, all genuine Shelly.
  https://www.proshop.dk/?s=shelly+plug
- Proshop `?s=shelly+kompatibel` → Shelly accessories, not a third-party clone shelf.
  https://www.proshop.dk/?s=shelly+kompatibel
- avXperten category "Shelly Smart Home Stikkontakt" contains genuine Shelly plus an
  **EcoFlow Stream Smart Plug — "Shelly Kompatibel", 2500W**, 359 DKK. The product page
  lists brand as both Shelly and EcoFlow.
  https://www.avxperten.dk/shelly-smart-home-stikkontakt
  https://www.avxperten.dk/ecoflow-stream-smart-plug-shelly-kompatibel-2500w.asp
- EcoFlow's page calls it "EcoFlow × Shelly Smart Plug", €34.90, 12 A, 44 × 44 × 70 mm,
  only compatible with EcoFlow STREAM. So this is a co-brand, not a counterfeit.
  https://eu.ecoflow.com/products/shelly-smart-plug
- Shelly's "Powered by Shelly" support article defines the official program.
  https://support.shelly.cloud/en/support/solutions/articles/103000406137-support-scope-for-powered-by-shelly-devices

## Practical search ladder

1. `smart stikkontakt energimåling` → broad Danish retail.
2. `stikkontakt effektmåler Matter` → modern ecosystem plugs.
3. `Zigbee stikkontakt energimåling` → hub-based plugs.
4. `Tasmota stikkontakt` / `ESPHome smart plug` → local-first, no cloud.
5. `Shelly kompatibel stikkontakt` → co-branded/compatibility results, not a brand list.
6. Add `site:proshop.dk`, `site:avxperten.dk`, `site:pricerunner.dk` to compare Danish
   stock and prices.

## Noise warning

`smart stikkontakt` on Proshop returned **4105 varer** because the search matches the
words loosely and pulls in cameras and other Smart Home gear. Add `energimåling`,
`effektmåler`, `Zigbee`, `Matter` or `16A` to narrow it. The raw count is not a product
count.
Source: https://www.proshop.dk/?s=smart+stikkontakt
