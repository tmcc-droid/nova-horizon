# How to work with the AI to build the game you want

You already have a **playable multiplayer core**. The best way to grow it is **small, clear requests** with **priorities**, not “build everything.”

## 1. Use a living wishlist

Keep a simple ordered list (chat is fine):

```text
P0 (must feel good this week)
- Radar/map
- Clear station names on map
- ...

P1 (next)
- Cargo hold UI
- Mission “fly to station” marker
- ...

P2 (later)
- Dynamic economy
- More ships
```

Update it after each play session. **You decide order; I implement.**

## 2. Best request format

For each feature, send:

1. **Player fantasy** — one sentence (“I want to always know where Earth Orbit is”)
2. **Must** — required behavior
3. **Nice** — optional polish
4. **Out of scope** — what not to do yet
5. **How I’ll test** — “log in, fly left, map shows station”

Example:

> **Minimap**  
> Fantasy: never get lost in Sol.  
> Must: show me, stations with names, other ships; range toggle.  
> Nice: click-to-set waypoint.  
> Out: galaxy map of all systems.  
> Test: undock, fly away, still see earth_orbit on radar.

## 3. Feedback after play (highest value)

After 10–15 minutes of play, reply with:

| Prompt | Example |
|--------|---------|
| Confusing | “Didn’t know Mars was that far” |
| Broken | “Dock says TooFar when I’m on top of it” |
| Fun | “Pirate fights feel good with juice” |
| Priority next | “Cargo UI > more weapons” |

Screenshots or short clips help, but text is enough.

## 4. How we ship changes

Prefer **vertical slices**:

- One feature playable end-to-end (client + server if needed)
- Then the next

Avoid parallel “do all systems” unless you explicitly want a batch week.

## 5. What only you can decide

I should not invent these without you:

- Tone (hardcore sim vs arcade)
- PvP rules (open, safe zones only, etc.)
- Progression (grind, story, cosmetics)
- Art direction (more cartoony vs realistic)
- Scope of “done” for v0.1 / friends alpha

When you’re unsure, give **two options** and pick one.

## 6. What I handle well alone

- Netcode / server authority
- UI layout and juice
- Content TOML (stations, ships, prices)
- Bugs you can reproduce
- Refactors behind a working feature

## 7. Cadence that works

1. You play 10 minutes  
2. You send 3 bullets: bug / want / priority  
3. I implement one slice  
4. You reload Godot and verify  
5. Repeat  

## 8. Current product north star (editable)

**Nova Horizon v0.1 friends alpha:**  
2 systems, trade loop, combat with pirates, radar, dock/refuel/jump, looks intentional, multiplayer stable for 2–4 friends.

Change this paragraph anytime — it steers everything else.
