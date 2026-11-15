//! The commit: every check, then all the staged changes at once or none
//! (`docs/regis-v2.md`, "Commit and repaint" and "Lifetime and memory").

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use super::stage::Stage;
use super::{Scene, Tables};
use crate::event::CommitError;
use crate::limits::{DRAWING_VISIT_COST, MAX_DRAWING_DEPTH, MAX_REBUILD_WORK};
use crate::machine::Machine;

/// A commit's failure: why, and the drawing concerned or 0.
type Failure = (CommitError, u16);

/// A drawing's place in the search for rings and depth.
#[derive(Clone, Copy)]
enum Seen {
    /// On the path being followed.
    Open,
    /// Finished: how many drawings deep it nests, itself included.
    Depth(usize),
}

/// The drawings painted as part of painting `number`: its source, if that
/// exists, and its children.
fn nested(stage: &Stage, number: u16) -> Vec<u16> {
    let mut nested = stage.children(number);
    if let Some(source) = stage.get(number).map(|drawing| drawing.source)
        && stage.get(source).is_some()
    {
        nested.push(source);
    }
    nested
}

/// Checks that no drawing is reached from itself through parent and source
/// links, and that none nests deeper than the cap. The walk keeps its own
/// stack, at most one entry deeper than the cap.
fn check_nesting(stage: &Stage) -> Result<(), Failure> {
    let mut seen: BTreeMap<u16, Seen> = BTreeMap::new();
    for root in stage.numbers() {
        if seen.contains_key(&root) {
            continue;
        }
        // Each entry: a drawing, what nests in it, how far through those
        // the walk is, and the deepest of them so far.
        let mut path: Vec<(u16, Vec<u16>, usize, usize)> = Vec::new();
        seen.insert(root, Seen::Open);
        path.push((root, nested(stage, root), 0, 0));
        while let Some((number, inner, next, deepest)) = path.last_mut() {
            let number = *number;
            let Some(child) = inner.get(*next).copied() else {
                let depth = deepest.saturating_add(1);
                seen.insert(number, Seen::Depth(depth));
                path.pop();
                if let Some((_, _, _, deepest)) = path.last_mut() {
                    *deepest = (*deepest).max(depth);
                }
                continue;
            };
            *next = next.saturating_add(1);
            match seen.get(&child).copied() {
                Some(Seen::Open) => return Err((CommitError::Cycle, child)),
                Some(Seen::Depth(depth)) => *deepest = (*deepest).max(depth),
                None => {
                    seen.insert(child, Seen::Open);
                    path.push((child, nested(stage, child), 0, 0));
                }
            }
            // A path this long is already too deep, whatever it leads to.
            let longest = path.iter().map(|(_, _, _, deepest)| deepest.saturating_add(1)).max().unwrap_or(0);
            if path.len() > MAX_DRAWING_DEPTH || longest > MAX_DRAWING_DEPTH {
                return Err((CommitError::Depth, number));
            }
        }
    }
    // The deepest chain may end at a drawing finished on an earlier path.
    for (number, state) in &seen {
        if let Seen::Depth(depth) = state
            && *depth > MAX_DRAWING_DEPTH
        {
            return Err((CommitError::Depth, *number));
        }
    }
    Ok(())
}

impl Scene {
    /// `D(C)`: applies the staged operations, or fails and drops them.
    pub(super) fn commit(&mut self, base: &mut Machine) -> Result<(), Failure> {
        let batch = self.batch.take();
        let staged_bytes = core::mem::take(&mut self.staged_bytes);
        let result = self.check(batch.as_ref().and_then(|batch| batch.failure), staged_bytes, base);
        match result {
            Ok(tables) => {
                self.tables = tables;
                self.committed = self.staged.clone();
                self.commits = self.commits.wrapping_add(1);
                Ok(())
            }
            Err(failure) => {
                // Nothing changes: the staged scene goes back to what is
                // on show, the player to the committed tables, and a
                // change to the output map made in the batch is dropped.
                self.staged = self.committed.clone();
                self.load_tables();
                if let Some(batch) = batch {
                    base.palette = batch.palette;
                }
                Err(failure)
            }
        }
    }

    /// Every check of a commit; the tables it would commit.
    fn check(&mut self, failure: Option<Failure>, staged_bytes: usize, base: &Machine) -> Result<Tables, Failure> {
        if let Some(failure) = failure {
            return Err(failure);
        }
        if staged_bytes > Self::staged_cap() {
            return Err((CommitError::Staged, 0));
        }
        for (number, drawing) in self.staged.drawings() {
            if drawing.parent != 0 && self.staged.get(drawing.parent).is_none() {
                return Err((CommitError::Parent, number));
            }
        }
        check_nesting(&self.staged)?;
        // The drawings use the tables as they stand now; if those changed,
        // every drawing's work is measured again.
        let tables = Tables::of(base);
        if tables != self.tables {
            self.staged.forget_work();
            self.player.machine.macrographs.clone_from(&tables.macrographs);
            self.player.machine.characters.sets.clone_from(&tables.sets);
        }
        self.check_work()?;
        Ok(tables)
    }

    /// Gives the player the committed tables.
    fn load_tables(&mut self) {
        self.player.machine.macrographs.clone_from(&self.tables.macrographs);
        self.player.machine.characters.sets.clone_from(&self.tables.sets);
    }

    /// Checks that painting every visible drawing costs no more than the
    /// cap, measuring the contents not yet measured.
    fn check_work(&mut self) -> Result<(), Failure> {
        let mut costs = BTreeMap::new();
        let mut total = 0_u64;
        for number in self.staged.children(0) {
            if self.staged.get(number).is_some_and(|drawing| drawing.visible) {
                total = total.saturating_add(self.cost(number, &mut costs, MAX_DRAWING_DEPTH)?);
                if total > MAX_REBUILD_WORK {
                    return Err((CommitError::Work, 0));
                }
            }
        }
        Ok(())
    }

    /// The work of painting one drawing where it is used: itself, its
    /// contents or its source, and its visible children. A source counts
    /// each time it is used. The nesting check has bounded `depth`.
    fn cost(&mut self, number: u16, costs: &mut BTreeMap<u16, u64>, depth: usize) -> Result<u64, Failure> {
        if let Some(cost) = costs.get(&number) {
            return Ok(*cost);
        }
        let Some(depth) = depth.checked_sub(1) else { return Err((CommitError::Depth, number)) };
        let Some(source) = self.staged.get(number).map(|drawing| drawing.source) else { return Ok(0) };
        let mut cost = DRAWING_VISIT_COST;
        cost = cost.saturating_add(match source {
            0 => self.contents_work(number)?,
            // A source that does not exist draws nothing.
            source if self.staged.get(source).is_none() => 0,
            source => self.cost(source, costs, depth)?,
        });
        for child in self.staged.children(number) {
            if self.staged.get(child).is_some_and(|drawing| drawing.visible) {
                cost = cost.saturating_add(self.cost(child, costs, depth)?);
            }
        }
        if cost > MAX_REBUILD_WORK {
            return Err((CommitError::Work, 0));
        }
        costs.insert(number, cost);
        Ok(cost)
    }

    /// The work a drawing's own contents cost, measured once.
    fn contents_work(&mut self, number: u16) -> Result<u64, Failure> {
        let Some(drawing) = self.staged.get(number) else { return Ok(0) };
        if let Some(work) = drawing.work {
            return Ok(work);
        }
        let contents = drawing.contents.clone();
        let work = self.measure(&contents).ok_or((CommitError::Work, 0))?;
        if let Some(drawing) = self.staged.get_mut(number) {
            drawing.work = Some(work);
        }
        Ok(work)
    }
}
