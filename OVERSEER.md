# Overseer instructions

Read by the overseer session only. CLAUDE.md is for every agent; this file is
for the one that talks to the owner.

## How to work with the owner (2026-09-02)

- Ask questions only through the AskUserQuestion tool. Never leave a question,
  an "I would want to know X first", or an undecided option list in a response.
- Push back on every decision where a better option exists, even a slightly
  better one. Give harsh, direct assessments. Verify a claim by running the
  code or a throwaway test before making it.
- Do not write to the Claude memory directory. Everything persistent lives in
  this repo, where the owner can review it.
- Commit only on the owner's explicit word, once per commit. There is no
  standing authorization in this repo.

## Rulings not yet folded into the design doc (2026-09-02)

These move into the design doc when it is reconciled; delete them here then.

- Numerics: f64, with the pure-Rust `libm` crate for transcendentals. Forbid
  std float transcendentals by lint. The sim owns its vector type with a
  closed op set; conversion to the engine's f32 math happens at the render
  wall.
- No speed limit of any kind, per row or global. Sim tick rate is 120 per
  second from the start.
- Targets: native and web from the first commit. Networking is one
  WebSocket relay protocol that both targets use.
- Never a 2D prototype. The playable runs on the mirage-renderer engine at
  `../../mirage-renderer` relative to this repo, as a path dependency.
- The sim step reads an immutable state and produces effects that apply at
  the end of the step, so a unit destroyed this tick still acts this tick.
  Targeting reads the snapshot plus a table of damage already assigned this
  tick, shooters resolve in sub-tick ready-time order then id, and a target
  whose assigned damage is lethal is skipped.
- The prototype from the web sessions was moved out of the repo to
  `/tmp/probe-game-previous-work` on 2026-09-02. The owner does not need
  it kept; the two design documents in it are the only written spec until
  the new design doc exists.
- The Linear issue list is deferred; it is not part of the start.
