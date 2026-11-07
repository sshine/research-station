# Source note — Symantec, W32.Stuxnet Dossier (2011)

- **Title:** W32.Stuxnet Dossier, Version 1.4
- **Authors:** Nicolas Falliere, Liam O Murchu and Eric Chien
- **Publisher:** Symantec Security Response
- **Published:** February 2011 (v1.0 September 2010; v1.4 February 2011)
- **URL:** https://nsarchive2.gwu.edu/NSAEBB/NSAEBB424/docs/Cyber-044.pdf
- **Source type:** primary technical analysis by the security firm that reverse-engineered the worm
- **Retrieved:** 2026-11-07

## Why it matters

The technical reference for what Stuxnet actually did: four zero-day exploits, two compromised certificates, a Windows rootkit, and a PLC payload that manipulated Siemens frequency-converter drives while hiding the changes from operators.

## Key findings (paraphrased from the dossier)

- Stuxnet is "the first to exploit four 0-day vulnerabilities compromise two digital certificates and inject code into industrial control systems and hide the code from the operator" (Summary).
- The PLC payload targets Siemens S7-315 and S7-417 controllers and their frequency-converter drives. It monitors normal operating frequencies between 807 Hz and 1210 Hz, then sends sequences that set the drive to 1410 Hz, then 2 Hz, then 1064 Hz, "slowing down or speeding up the motor to different rates at different times" (Modifying PLCs section).
- The Windows component hooks Siemens Step 7 APIs so that the monitoring system (WinCC) receives the previous, normal operating frequencies during sabotage; the operator does not see the anomalous values.
- Removable-drive propagation uses a malicious LNK file and a rootkit to hide files on USB drives; the worm also spreads over local networks and shared printers.
- The dossier's sequence C discussion refers to "six cascades containing 164 centrifuges each" and "a total of 110 peripherals will be affected out of 164"; sequence C was incomplete, so this is intent rather than a confirmed effect.

## Exact quote (Summary, OCR punctuation normalised)

> Stuxnet represents the first of many milestones in malicious code history – it is the first to exploit four 0-day vulnerabilities compromise two digital certificates and inject code into industrial control systems and hide the code from the operator.

## Notes and caveats

- Symantec analysed the code; it did not attribute authorship. Attribution comes from the journalism cited elsewhere in this bundle.
- The dossier is a PDF; the OCR text has no punctuation in places, so quotes above are normalised and should be treated as such.
- The frequency values are strong technical evidence that the payload was designed for centrifuge drives, not generic sabotage.
