# Overseer instructions

Read by the overseer session only. CLAUDE.md is for every agent; this file is
for the one that talks to the owner. Project state lives in TODO.md.

## How to work with the owner (2026-09-02)

- Slow and steady, in every session (owner, 2026-09-05). There is no
  rush. Confirm design with the owner before building on it: a design
  change is stated as mechanism, read by the owner, and ruled, before a
  brief carries it. Agents must stop and ask more often than they do: an
  agent's question comes to the overseer as a stop-and-report, and the
  overseer puts it to the owner through the question tool; the overseer
  never answers a design question on the owner's behalf.
- Ask questions only through the AskUserQuestion tool. Never leave a question,
  an "I would want to know X first", or an undecided option list in a response.
- Push back on every decision where a better option exists, even a slightly
  better one. Give harsh, direct assessments. Verify a claim by running the
  code or a throwaway test before making it.
- Work can be left for the future (owner, 2026-09-06). A measured cost or
  a better shape found mid-unit is recorded as an open item, not folded
  into the unit in flight; the unit lands at its briefed scope.
- A code shape the owner finds is not fed to a reviewer (owner,
  2026-09-06): a blind Sonnet review of the unit runs first, and whether
  it finds the same shape is the measure of its report.
- Do not write to the Claude memory directory. Everything persistent lives in
  this repo, where the owner can review it.
- Commit only on the owner's explicit word, once per commit. There is no
  standing authorization in this repo. Stop before a commit so the owner can
  verify the working tree.
- A question goes on the question tool in the same turn it arises, never
  deferred to a later turn (owner, 2026-09-06). Text in the same turn as
  a tool call is collapsed in the terminal, so the reading before a
  question is kept short enough to survive that.
- The owner reads design in full. Write the mechanism: what is drawn, where,
  when it changes. Never imagery, never a summary of a mechanism.
- Design conversations run on the owner's questions: answer the question
  asked, at the level asked. When the owner says a shape is too complex, the
  answer is a simpler shape, not a defence.
- The engine is being polished through this game. Report a needed engine
  feature or an API friction to the owner clearly and at once, as its own
  item, never folded into a workaround.
