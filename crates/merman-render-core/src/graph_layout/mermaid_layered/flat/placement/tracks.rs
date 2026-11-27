//! Stepped tracks and curved levels between rank bands.

use super::bands::Piece;
use super::{GeoResult, IN_LINE, Work, compare_number, inconsistent, js_max, js_min};

const MAX_WEIGHED: usize = 400;
const LEVEL_LEAD: f64 = 12.0;
const LEVEL_CLEAR: f64 = 8.0;
const LEVEL_TOUCH: f64 = 4.0;
const CROSSED_SLOPE: f64 = 0.5;
const SAMPLES: usize = 16;

pub(super) struct Tracks {
    pub count: Vec<usize>,
    pub tall: Vec<f64>,
}

pub(super) fn assign<E>(
    pieces: &mut [Piece],
    room: &[f64],
    stepped: bool,
    work: &mut Work<'_, E>,
) -> GeoResult<Tracks, E> {
    let mut result = Tracks {
        count: work.filled(room.len(), 0)?,
        tall: work.filled(room.len(), 0.0)?,
    };
    let mut ids = work.reserve(pieces.len())?;
    let mut intervals = work.reserve(pieces.len())?;
    work.price(pieces.len())?;
    for (id, piece) in pieces.iter().enumerate() {
        work.poll(id)?;
        if (piece.from - piece.to).abs() >= IN_LINE {
            ids.push(id);
        }
        intervals.push(Interval {
            lo: js_min(piece.from, piece.to),
            hi: js_max(piece.from, piece.to),
        });
    }
    // A gap only mutates its own pieces. Rank processing order is immaterial to its result;
    // the edges within each gap retain creation order until lay sorts the local list.
    work.sort(&mut ids, |a, b| {
        (pieces[a].rank, a).cmp(&(pieces[b].rank, b))
    })?;
    let mut planner = Planner {
        pieces,
        intervals: &intervals,
    };
    let mut start = 0;
    work.price(ids.len())?;
    while start < ids.len() {
        work.poll(start)?;
        let rank = planner.pieces[ids[start]].rank;
        let mut end = start + 1;
        while end < ids.len() && planner.pieces[ids[end]].rank == rank {
            work.poll(end - start)?;
            end += 1;
        }
        let list = &mut ids[start..end];
        if stepped {
            result.count[rank] = planner.lay(list, false, work)?;
        } else if list.len() <= MAX_WEIGHED {
            let (count, tall) = planner.curved(list, room[rank], work)?;
            result.count[rank] = count;
            result.tall[rank] = tall;
        }
        start = end;
    }
    Ok(result)
}

#[derive(Clone, Copy)]
struct Interval {
    lo: f64,
    hi: f64,
}

struct Planner<'a> {
    pieces: &'a mut [Piece],
    intervals: &'a [Interval],
}

impl Planner<'_> {
    fn within(&self, x: f64, k: usize) -> usize {
        let range = self.intervals[k];
        usize::from(x >= range.lo - IN_LINE && x <= range.hi + IN_LINE)
    }
    fn mates(&self, a: usize, b: usize) -> bool {
        let (a, b) = (self.pieces[a], self.pieces[b]);
        (a.from - b.from).abs() < IN_LINE || (a.to - b.to).abs() < IN_LINE
    }
    fn overlap(&self, a: usize, b: usize) -> bool {
        let (a, b) = (self.intervals[a], self.intervals[b]);
        a.lo < b.hi - IN_LINE && b.lo < a.hi - IN_LINE
    }
    fn clash(&self, a: usize, b: usize) -> bool {
        self.overlap(a, b) && !self.mates(a, b)
    }
    fn meet(&self, a: usize, b: usize) -> bool {
        self.overlap(a, b) || self.crossings(a, b) + self.crossings(b, a) > 0
    }
    fn crossings(&self, a: usize, b: usize) -> usize {
        self.within(self.pieces[a].to, b) + self.within(self.pieces[b].from, a)
    }
    fn directs_right(&self, k: usize) -> bool {
        self.pieces[k].from < self.pieces[k].to
    }

    fn near_pairs<E>(
        &self,
        list: &[usize],
        work: &mut Work<'_, E>,
    ) -> GeoResult<Vec<(usize, usize)>, E> {
        let mut by_lo = work.indices(list.len())?;
        work.sort(&mut by_lo, |a, b| {
            compare_number(self.intervals[list[a]].lo, self.intervals[list[b]].lo).then(a.cmp(&b))
        })?;
        let mut pairs = Vec::new();
        work.price(by_lo.len())?;
        for (x, &i) in by_lo.iter().enumerate() {
            work.poll(x)?;
            let reach = self.intervals[list[i]].hi + 4.0 * IN_LINE;
            for &j in &by_lo[x + 1..] {
                if self.intervals[list[j]].lo > reach {
                    break;
                }
                work.push(&mut pairs, (i.min(j), i.max(j)))?;
            }
        }
        Ok(pairs)
    }

    fn lay<E>(
        &mut self,
        list: &mut [usize],
        curved: bool,
        work: &mut Work<'_, E>,
    ) -> GeoResult<usize, E> {
        if list.is_empty() {
            return Ok(0);
        }
        work.sort(list, |a, b| {
            let (a_range, b_range) = (self.intervals[a], self.intervals[b]);
            // The source compares the two interval widths with successive subtractions.
            compare_number(b_range.hi - b_range.lo - (a_range.hi - a_range.lo), 0.0)
                .then_with(|| compare_number(a_range.lo, b_range.lo))
                .then(a.cmp(&b))
        })?;
        if list.len() <= MAX_WEIGHED {
            self.weighed(list, curved, work)
        } else {
            self.plain(list, work)
        }
    }

    fn plain<E>(&mut self, list: &[usize], work: &mut Work<'_, E>) -> GeoResult<usize, E> {
        let mut rows: Vec<Vec<usize>> = work.reserve(list.len())?;
        work.price(list.len())?;
        for (index, &piece) in list.iter().enumerate() {
            work.poll(index)?;
            let mut track = 0;
            'find: for row in &rows {
                work.price(1)?;
                for &other in row {
                    work.price(1)?;
                    if self.clash(piece, other) {
                        track += 1;
                        continue 'find;
                    }
                }
                break;
            }
            if track == rows.len() {
                rows.push(Vec::new());
            }
            work.push(&mut rows[track], piece)?;
            self.pieces[piece].track = Some(track);
            self.pieces[piece].levels = 1;
        }
        Ok(rows.len())
    }

    fn weighed<E>(
        &mut self,
        list: &[usize],
        curved: bool,
        work: &mut Work<'_, E>,
    ) -> GeoResult<usize, E> {
        let pairs = self.near_pairs(list, work)?;
        let count = list.len();
        let mut group = work.indices(count)?;
        let mut above = empty_rows(count, work)?;
        let mut after = empty_rows(count, work)?;
        let mut clashes = empty_rows(count, work)?;
        let mut waiting = work.filled(count, 0i32)?;
        work.price(pairs.len())?;
        for (slot, (i, j)) in pairs.into_iter().enumerate() {
            work.poll(slot)?;
            let (a, b) = (list[i], list[j]);
            if self.clash(a, b) {
                work.push(&mut clashes[i], j)?;
                work.push(&mut clashes[j], i)?;
            }
            if !self.meet(a, b) {
                continue;
            }
            let first_root = root(&mut group, i, work)?;
            group[first_root] = root(&mut group, j, work)?;
            let (ab, ba) = (self.crossings(a, b), self.crossings(b, a));
            if ab == ba {
                continue;
            }
            let (first, second) = if ab < ba { (i, j) } else { (j, i) };
            work.push(&mut above[second], first)?;
            work.push(&mut after[first], second)?;
            waiting[second] += 1;
        }
        let mut placed = work.filled(count, false)?;
        let mut sequence = work.reserve(count)?;
        for _ in 0..count {
            work.price(count)?;
            let mut pick = None;
            for i in 0..count {
                work.poll(i)?;
                if placed[i] {
                    continue;
                }
                if pick.is_none_or(|p| waiting[i] < waiting[p]) {
                    pick = Some(i);
                }
                if waiting[i] == 0 {
                    break;
                }
            }
            let pick = pick.ok_or(inconsistent("no eligible piece for a route track"))?;
            placed[pick] = true;
            sequence.push(pick);
            work.price(after[pick].len())?;
            for (index, &j) in after[pick].iter().enumerate() {
                work.poll(index)?;
                waiting[j] -= 1;
            }
        }
        // The next track cannot be higher than one past the largest assigned track.
        // Even when the preference graph has cycles, `count` slots are sufficient.
        let mut taken = work.filled(count, 0usize)?;
        let mut rows = 0;
        work.price(sequence.len())?;
        for (index, at) in sequence.into_iter().enumerate() {
            work.poll(index)?;
            let mut track = 0;
            work.price(above[at].len())?;
            for (j, &i) in above[at].iter().enumerate() {
                work.poll(j)?;
                if let Some(other) = self.pieces[list[i]].track {
                    track = track.max(other + 1);
                }
            }
            work.price(clashes[at].len())?;
            for (j, &i) in clashes[at].iter().enumerate() {
                work.poll(j)?;
                if let Some(other) = self.pieces[list[i]].track {
                    taken[other] = index + 1;
                }
            }
            while taken.get(track) == Some(&(index + 1)) {
                work.price(1)?;
                track += 1;
            }
            if track >= count {
                return Err(inconsistent("invalid route track membership"));
            }
            self.pieces[list[at]].track = Some(track);
            rows = rows.max(track + 1);
        }
        let mut first = work.filled(count, rows)?;
        let mut last = work.filled(count, 0)?;
        work.price(count)?;
        for (i, &id) in list.iter().enumerate() {
            work.poll(i)?;
            let root = root(&mut group, i, work)?;
            let track = self.pieces[id]
                .track
                .ok_or(inconsistent("invalid route track preference"))?;
            first[root] = first[root].min(track);
            last[root] = last[root].max(track);
        }
        work.price(count)?;
        for (i, &id) in list.iter().enumerate() {
            work.poll(i)?;
            let root = root(&mut group, i, work)?;
            self.pieces[id].levels = last[root] - first[root] + 1;
            if curved {
                let track = self.pieces[id]
                    .track
                    .ok_or(inconsistent("invalid route track preference"))?;
                self.pieces[id].track = Some(track - first[root]);
            }
        }
        Ok(rows)
    }

    fn curved<E>(
        &mut self,
        all: &[usize],
        room: f64,
        work: &mut Work<'_, E>,
    ) -> GeoResult<(usize, f64), E> {
        let mut far = work.reserve(all.len())?;
        work.price(all.len())?;
        for (i, &id) in all.iter().enumerate() {
            work.poll(i)?;
            if self.pieces[id].wide {
                far.push(id);
            }
        }
        self.lay(&mut far, true, work)?;
        let mut marks = work.filled(all.len(), Marks::default())?;
        work.price(all.len())?;
        for (i, &id) in all.iter().enumerate() {
            work.poll(i)?;
            marks[i].level = self.pieces[id].wide && self.pieces[id].levels > 1;
        }
        let pairs = self.near_pairs(all, work)?;
        work.price(pairs.len())?;
        for (index, &(i, j)) in pairs.iter().enumerate() {
            work.poll(index)?;
            let (a, b) = (all[i], all[j]);
            if (marks[i].level && marks[j].level)
                || !self.clash(a, b)
                || self.directs_right(a) != self.directs_right(b)
            {
                continue;
            }
            let limit = if self.pieces[a].wide || self.pieces[b].wide {
                LEVEL_CLEAR
            } else {
                LEVEL_TOUCH
            };
            if near(self.pieces[a], self.pieces[b], room, limit, work)? {
                marks[i].level = true;
                marks[j].level = true;
            }
        }
        work.price(far.len())?;
        for (i, id) in far.into_iter().enumerate() {
            work.poll(i)?;
            self.pieces[id].track = None;
            self.pieces[id].levels = 0;
        }
        if !marks.iter().any(|mark| mark.level) {
            return Ok((0, 0.0));
        }
        let mut crossing = 0;
        let mut alongside = 0;
        work.price(pairs.len())?;
        for (slot, (i, j)) in pairs.into_iter().enumerate() {
            work.poll(slot)?;
            let (a, b) = (all[i], all[j]);
            if marks[i].level && marks[j].level {
                if !self.overlap(a, b) {
                    continue;
                }
                if self.directs_right(a) != self.directs_right(b) {
                    crossing += 1;
                } else if !self.mates(a, b) {
                    alongside += 1;
                }
            } else if (marks[i].level || marks[j].level) && self.meet(a, b) {
                marks[i].beside = true;
                marks[j].beside = true;
            }
        }
        if crossing > alongside {
            let mut tall = 0.0;
            work.price(all.len())?;
            for (i, &id) in all.iter().enumerate() {
                work.poll(i)?;
                let interval = self.intervals[id];
                let h = (interval.hi - interval.lo) * CROSSED_SLOPE;
                if h > tall {
                    tall = h;
                }
            }
            return Ok((0, tall));
        }
        let mut members = work.reserve(all.len())?;
        work.price(all.len())?;
        for (i, &id) in all.iter().enumerate() {
            work.poll(i)?;
            if marks[i].level || (!self.pieces[id].wide && marks[i].beside) {
                members.push(id);
            }
        }
        self.lay(&mut members, true, work)?;
        let mut most = 0;
        work.price(members.len())?;
        for (i, &id) in members.iter().enumerate() {
            work.poll(i)?;
            most = most.max(self.pieces[id].levels);
        }
        Ok((most, 0.0))
    }
}

#[derive(Clone, Copy, Default)]
struct Marks {
    level: bool,
    beside: bool,
}

fn empty_rows<E>(len: usize, work: &mut Work<'_, E>) -> GeoResult<Vec<Vec<usize>>, E> {
    let mut items = work.reserve(len)?;
    work.price(len)?;
    for i in 0..len {
        work.poll(i)?;
        items.push(Vec::new());
    }
    Ok(items)
}

fn root<E>(groups: &mut [usize], mut i: usize, work: &mut Work<'_, E>) -> GeoResult<usize, E> {
    while groups[i] != i {
        work.price(1)?;
        groups[i] = groups[groups[i]];
        i = groups[i];
    }
    Ok(i)
}

fn near<E>(
    a: Piece,
    b: Piece,
    height: f64,
    limit: f64,
    work: &mut Work<'_, E>,
) -> GeoResult<bool, E> {
    let h = js_max(height / 2.0, height - 2.0 * LEVEL_LEAD);
    work.price(2 * (SAMPLES + 1))?;
    let a = sample(a, h);
    let b = sample(b, h);
    if reaches(&a, &b, limit, work)? {
        return Ok(true);
    }
    reaches(&b, &a, limit, work)
}

fn sample(piece: Piece, h: f64) -> [(f64, f64); SAMPLES + 1] {
    let across = (piece.to - piece.from).abs();
    let divisor = if across == 0.0 { 1.0 } else { across };
    let ease = js_max(0.25, js_min(0.5, 4.0 * h.abs() / divisor));
    std::array::from_fn(|i| {
        let t = i as f64 / SAMPLES as f64;
        let u = 1.0 - t;
        (
            piece.from + (piece.to - piece.from) * t * t * (3.0 - 2.0 * t),
            h * (3.0 * ease * t * u * u + 3.0 * (1.0 - ease) * t * t * u + t * t * t),
        )
    })
}

fn reaches<E>(
    points: &[(f64, f64)],
    line: &[(f64, f64)],
    limit: f64,
    work: &mut Work<'_, E>,
) -> GeoResult<bool, E> {
    let bound = limit * limit * (1.0 + 1e-9);
    for &(px, py) in points {
        work.price(SAMPLES)?;
        for pair in line.windows(2) {
            let (x, y) = pair[0];
            let (dx, dy) = (pair[1].0 - x, pair[1].1 - y);
            let norm = dx * dx + dy * dy;
            let div = if norm == 0.0 || norm.is_nan() {
                1.0
            } else {
                norm
            };
            let t = js_max(0.0, js_min(1.0, ((px - x) * dx + (py - y) * dy) / div));
            let (ex, ey) = (px - x - t * dx, py - y - t * dy);
            if ex * ex + ey * ey < bound && ex.hypot(ey) < limit {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
