//! The repaint budget (`docs/regis-v2.md`, "Repaint budget"; REQ-6368).

use regis::limits::{MAX_REBUILD_WORK, REPAINT_WORK_HELD, REPAINT_WORK_PER_SECOND};
use regis::{Event, Picture, RepaintBudget, Screen, Terminal};

/// A budget with nothing in hand.
fn emptied() -> RepaintBudget {
    let mut budget = RepaintBudget::new();
    budget.spend(REPAINT_WORK_HELD);
    budget
}

#[test]
fn a_second_in_millisecond_steps_earns_exactly_a_seconds_work() {
    let mut whole = emptied();
    whole.spend(REPAINT_WORK_PER_SECOND);
    let mut stepped = whole.clone();
    whole.earn(1000);
    for _ in 0..1000 {
        stepped.earn(1);
    }
    assert_eq!(stepped, whole);
    assert!(!whole.may_repaint(), "exactly what was owed is earned: nothing is in hand");
    whole.earn(1);
    assert!(whole.may_repaint());
}

#[test]
fn no_more_than_the_held_allowance_is_ever_saved() {
    let mut budget = RepaintBudget::new();
    budget.earn(3_600_000);
    assert_eq!(budget, RepaintBudget::new());
    budget.spend(REPAINT_WORK_HELD);
    assert!(!budget.may_repaint(), "an hour's idleness bought one held allowance and no more");
}

#[test]
fn a_repaint_that_started_is_paid_for_in_full_and_earned_back() {
    let mut budget = RepaintBudget::new();
    assert!(budget.may_repaint());
    // The held allowance is two repaints at the cap; a second that costs
    // a little more than is left still runs, and overdraws.
    budget.spend(MAX_REBUILD_WORK);
    assert!(budget.may_repaint());
    budget.spend(MAX_REBUILD_WORK + 5);
    assert!(!budget.may_repaint());
    let wait = budget.milliseconds_until_repaint();
    assert!(wait > 0);
    let mut early = budget.clone();
    early.earn(wait - 1);
    assert!(!early.may_repaint(), "the wait given is the least");
    budget.earn(wait);
    assert!(budget.may_repaint());
    assert_eq!(budget.milliseconds_until_repaint(), 0);
}

#[test]
fn the_wait_counts_what_is_part_earned() {
    // 0.3 milliseconds' worth short: one millisecond is enough, however
    // the time before was divided.
    let mut budget = emptied();
    budget.spend(REPAINT_WORK_PER_SECOND / 1000 * 7);
    for _ in 0..6 {
        budget.earn(1);
    }
    assert_eq!(budget.milliseconds_until_repaint(), 2);
    budget.earn(2);
    assert!(budget.may_repaint());
}

/// The commits of the test of minimal commits.
const COMMITS: u64 = 200;
/// The simulated time between them.
const MILLISECONDS_APART: u64 = 5;

#[test]
fn minimal_commits_cannot_buy_more_repaint_work_than_the_budget_allows() {
    // A scene near the cap of one repaint: a filled screen used 100 times.
    let master = "D(B1)F(P[0,0]V[799,0][799,479][0,479])D(E)D(S1(V0))";
    let uses = (0..100).map(|n| format!("D(S{}(R1))", n + 2)).collect::<Vec<_>>().concat();
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let events = terminal.feed_regis(&mut screen, format!("{master}{uses}D(C)").as_bytes());
    assert_eq!(events, [Event::Committed(1)]);

    // Commits of a dozen bytes, five milliseconds apart, each followed by
    // a wish to repaint: one simulated second.
    let seconds = COMMITS * MILLISECONDS_APART / 1000;
    // The budget starts with nothing held, which leaves the held allowance
    // out of the bound and the test with less to paint.
    let bound = seconds * REPAINT_WORK_PER_SECOND + MAX_REBUILD_WORK;
    let mut budget = emptied();
    let mut picture = Picture::new();
    let (mut spent, mut repaints) = (0_u64, 0_u32);
    for commit in 0..COMMITS {
        let events = terminal.feed_regis(&mut screen, format!("D(S2(X{}),C)", commit % 700).as_bytes());
        assert!(matches!(events[..], [Event::Committed(_)]));
        budget.earn(MILLISECONDS_APART);
        if budget.may_repaint() {
            terminal.compose(&screen, &mut picture);
            budget.spend(picture.work());
            spent += picture.work();
            repaints += 1;
            assert!(spent <= bound, "{repaints} repaints cost {spent}, over the bound of {bound}");
        }
    }
    assert!(repaints >= 2, "the budget lets repaints through: {repaints}");
    assert!(u64::from(repaints) < COMMITS / 10, "and holds most back: {repaints}");

    // The repaint owed is made when the budget allows, and shows the last
    // scene.
    budget.earn(budget.milliseconds_until_repaint());
    assert!(budget.may_repaint());
    terminal.compose(&screen, &mut picture);
    let mut fresh = Picture::new();
    terminal.compose(&screen, &mut fresh);
    assert_eq!(picture.canvas(), fresh.canvas());
    assert_eq!(u64::from(picture.commits()), COMMITS + 1);
}
