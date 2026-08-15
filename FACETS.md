# Facets

Design intent for control facet management in flowlang, captured from the
owner's design notes (2026-08-15). None of this is implemented yet; it is
the direction of record, deliberately parked outside the scope of the
current agent/memory cycle.

## Facets are code

A control's facets go well beyond the manual/memory records the current
agent work is concerned with. Facets carry:

- **Interface** — the UI components bound to a control: HTML, CSS,
  JavaScript, 3D assets.
- **Data contracts** — the data definitions contractually bound to the
  control ride in the data facet.
- **Source** — the command facet: the source the compiled commands were
  built from.
- **Manuals** — the memory records the agent work added.

All of it is as much the code as the compiled binary is. For a system
that understands itself, facets are first-class runtime information, and
manuals are first-class **at the flowlang level** — not an agent add-on.
Newbound without the agent still has its manual records; they are useful
in their own right and depend on nothing above flowlang.

## Elegant degradation

Libraries, cognition, and even memory recall should degrade elegantly, in
lockstep with two things: the capabilities of the machine, and the way
the library was delivered. Delivery mode defines which capabilities exist
at runtime — in theory all the way down to embedded no_std targets with
no facets at all.

Corollaries:

- Everything above the floor is opt-in. Newbound without agent is fully
  supported; a bare binary with no store is a valid delivery, and the
  system falls off gracefully to match it.
- The footprint floor is minimal: zero third-party crate dependencies.
- You should be able to select as much or as little of this as you want.

## Where a library's data lives (the three homes)

**1. Resident — the core/legacy case.** The library's data folder is
merged into the top-level `data/` folder of the running instance, as
today. Generated code targets a per-library crate: the legacy `cmd`
catch-all default gives way to a per-library-crate default (cleaner,
simpler, better — at least at the development stage). At publish time it
may still make sense to collapse every library into the `cmd` root and
deliver a single binary.

**2. FFI crate.** The crate root owns its own data folder —
`<crate root>/data/<library>` — and flowlang grows the ability to
**multi-home**: the compiled-in libraries' data home is the top-level
`data/` folder, and each FFI crate's root contributes its own data home
alongside it.

**3. crates.io dependency — unresolved.** When flowlang itself (or any
facet-bearing library) arrives as a registry dependency, its data folder
is not part of the consuming instance's store. The direction: some way to
pull those facets in and read them **read-only**. The exact mechanism is
the open design question below.

## The manual binary

The flowlang crate should ship a small binary target that prints out its
manual, so registry consumption still yields an accessible manual with
zero store plumbing — `man` for flowlang. (Name TBD.)

## Frictionless drop-in

Within pure-play flowlang, libraries must drop in as frictionlessly as
possible — and that requirement is amplified a thousandfold in the
newbound space. The bar: "start up newbound with these six libraries —
go." Any facet mechanism that adds per-library ceremony is wrong.

## The edges

The three homes are complete on their own axis — where the code came
from. The axis that actually drives behavior is **writability**: every
code source carries its data; the homes differ only in whether this
instance owns it (resident: yes; FFI crate in this checkout: yes;
registry dependency: no). The sharp edges the design will meet:

- **Git/path dependencies** sit between homes 2 and 3: the data folder
  is present on disk and readable, but not instance-owned. Under the
  writability framing they need no fourth home — a path-patched flowlang
  in a dev checkout is home 3 with a different mount path.
- **Precedence and collision.** Once multi-homing exists, two homes can
  claim the same library name (an FFI crate shipping a stale copy of a
  resident library, or vice versa). Resolution order must be declared —
  instance `data/` wins, then FFI homes, then read-only mounts — with a
  loud warning on shadowing.
- **Writes against read-only homes** need a defined refusal that
  redirects to the curated channel (deposit to the brain with a
  `subject`, promote where the subject's repo is writable), so the
  failure teaches the workflow instead of just erroring.
- **Version binding.** A read-only mount must serve the *pinned*
  version's data — the builder's core-pair resolution provides exactly
  that mapping. Embedding the manual in the crate at compile time
  sidesteps the question entirely: the binary carries its own version's
  manual, and the same bytes serve the single-binary publish mode.
- **Packaging intentionality.** The published crate ships `data/` today
  only because nothing excludes it — `data/mcp` and `data/testflow` ride
  along. Whatever ships becomes the read-only home's contents, so the
  package include list should be a decision, not a default.
- **ndata.** Manuals-first-class-at-the-flowlang-level leaves ndata
  homeless — it has no data folder anywhere. Either it grows the same
  embedded single-file manual, or its manual lives as a control on
  flowlang's store. Needs a call.
- **The no_std floor.** Facet access code itself must be feature-gated
  so the zero-dependency, no-facet floor genuinely holds.

## Open questions

1. For the agent work: do flowlang's facets need to be pulled into the
   consuming instance at all — and if so, through what read-only
   mechanism? (Current lean: read access via a compile-time-embedded
   manual, not residency; instance-authored claims about flowlang stay
   in the brain until promoted through a session with this repo
   attached.)
2. Do the three homes above cover every delivery contingency? (See "The
   edges" — the taxonomy holds; the writability rule and the edge
   policies above are what remain to pin down.)
