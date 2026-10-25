# A scripts' timer counts the game's elapsed minutes

Scripts (the runtime's `SetTimer`, after #726), 2026-10-25 (#933); the maintainer's decision.

## The original

`SetTimer` (56, `0x4982D0`) stores the moment it is set as `((((year·12 + month)·30 + day)·24 + hour)·60 + minute)` of the game's clock.
A timer runs once the clock's count, by the same formula, passes the moment by its time (docs/re/netscript.md, "`SetTimer`'s minutes and the calendar").
The count takes months of 30 days, but the date's carry makes eleven months of 29 days and the fifth of 34.
So a timer that runs across the end of a month runs a day early, across the fifth's end four days late, and across the year's end at once (read in the code, not yet seen in the running original).

## Ours

The runtime stores a timer's moment as the game's minutes elapsed since the new game, and a timer of a time `t` runs `t` minutes later, whatever the date.
The rest is the original's:
- a timer of the same id replaces the earlier one;
- at each minute that the clock completes, the first timer that is due runs its function, unless the function's quest is finished;
- every timer of that id is then removed.

## Why

The drift comes from the client's arithmetic, not from a rule of the play.
No quest's data or text relies on it: the scripts state their limits in days of the game, and a timer of three days should last three days in any month.
