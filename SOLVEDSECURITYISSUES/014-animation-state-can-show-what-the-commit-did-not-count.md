# 014 — The animation state can show drawings the commit's check of work did not count

**Found:** 18 November 2026, while building the animation state (REQ-6530; neoteletext-common `docs/animation.md`, ANIM-9).
**Class:** resource exhaustion (processor time), and a wrong picture.

A commit is refused when painting every visible drawing would cost more
than `MAX_REBUILD_WORK`, 100,000,000 work units (failed-commit code 6).
The check counts, for each drawing, the dearest of its looks and frames,
so that neither the viewer state nor the animation state's frame and
forced look can make a repaint cost more than the commit allowed. It does
not count a hidden drawing (`V0`): "hidden drawings cost storage but no
rendering work" (`docs/regis-v2.md`, "Bounds").

The animation state added on this day has a part the check knew nothing
of: *shown*, used in place of the attribute `V`. A receiver sets it for a
rule's `show` action or a channel's `ANIMATE` record, both of which come
from the page. A page could therefore commit many hidden drawings, each
just under the cap, and have 64 of them shown: a repaint asked to cost 64
times the cap.

The painter stops a repaint at the cap whatever the commit said, so the
time one repaint takes stayed bounded. What broke was the picture: a
whole repaint was cut short at the cap, while a partial repaint, which
pays only for the rectangle repainted, was not, so the two no longer gave
the same pixels; and every repaint of such a scene cost the whole cap.
ANIM-9's statement, "no animation state can make a repaint cost more than
the commit allowed", did not hold.

## Resolution (18 November 2026)

The check of work is made again with the drawings the animation state
shows counted as visible (`Scene::animation_work_fits`, which runs the
commit's own `check_work`):

- when `Terminal::set_animation` is given a state that shows a drawing:
  if the scene would then cost more than `MAX_REBUILD_WORK`, the state is
  kept without its shown part and the call returns false;
- after every commit, since a commit can make a shown drawing dearer:
  shown parts are dropped, highest drawing number first, until the scene
  fits.

A commit itself is still checked without the animation state, so that it
fails exactly when a sender's validator, which has no animation state,
says it will. Hidden drawings still cost no rendering work at a commit.

The other parts of the state cannot raise the cost: position, layer and
order do not change what is painted, and every frame and look was already
counted at the commit.

**Test:** `crates/regis/tests/animation.rs`,
`showing_hidden_drawings_cannot_take_a_repaint_over_the_cap`. With the
check removed it fails: all 64 drawings are shown, where the cap allows
25.

## Two costs looked at and found bounded

- **Showing a drawing starts its gradual painting** (ANIM-15), which means
  finding the drawings with a paint rate below it. Done at each
  `set_animation` that would be a walk of up to 65,535 drawings for each
  of 64 operations a tick. The drawings shown are only noted, and one walk
  of the scene finds them all when time next passes or a picture is made
  (`Scene::settle_starting`).
- **`Terminal::advance_ticks`** walks the scene's drawings once a call
  while any drawing is being painted gradually, and returns at once when
  none is: at most 65,535 visits a tick.
