# Weather

The rain, the fog and the snow (docs/re/weather.md).

| Decision | Ours, against the original | Why | Kind | Issues |
|---|---|---|---|---|
| Weather times span their range | A weather time is `shortest + (longest − shortest) · b / 255` and the snow goes on and off by its own flag, where the original's `/ b` nearly always gives the shortest, never ends for a byte of 0, and its snow never stops. | Both read as slips that no sector's entries ask for, and no rule of the play reads the weather; the maintainer's choice (#1083). | deviation | #1083 |
