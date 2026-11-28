use std::cmp::Ordering;

use super::{Charge, CompoundError, LevelPlan, LevelResult, Mode, Result, Solve};

const POLL_INTERVAL: usize = 256;

pub(super) fn add<E>(left: usize, right: usize) -> Result<usize, E> {
    left.checked_add(right)
        .ok_or(CompoundError::ArithmeticOverflow)
}

pub(super) fn mul<E>(left: usize, right: usize) -> Result<usize, E> {
    left.checked_mul(right)
        .ok_or(CompoundError::ArithmeticOverflow)
}

pub(super) fn geometry<E>(valid: bool) -> Result<(), E> {
    if valid {
        Ok(())
    } else {
        Err(CompoundError::NonFiniteGeometry)
    }
}

pub(super) fn check_slots<E>(len: usize) -> Result<(), E> {
    i32::try_from(len).map_err(|_| CompoundError::ArithmeticOverflow)?;
    Ok(())
}

pub(super) struct Work<'a, E> {
    charge: &'a mut Charge<'a, E>,
}

impl<'a, E> Work<'a, E> {
    pub(super) fn new(charge: &'a mut Charge<'a, E>) -> Self {
        Self { charge }
    }

    pub(super) fn price(&mut self, units: usize) -> Result<(), E> {
        (self.charge)(units).map_err(CompoundError::Work)
    }

    pub(super) fn scaled(&mut self, len: usize, units: usize) -> Result<(), E> {
        self.price(
            len.checked_mul(units)
                .ok_or(CompoundError::ArithmeticOverflow)?,
        )
    }

    pub(super) fn poll(&mut self, i: usize) -> Result<(), E> {
        if i.is_multiple_of(POLL_INTERVAL) {
            self.price(0)?;
        }
        Ok(())
    }

    pub(super) fn reserve<T>(&mut self, len: usize) -> Result<Vec<T>, E> {
        self.price(len)?;
        let mut items = Vec::new();
        items
            .try_reserve_exact(len)
            .map_err(|_| CompoundError::AllocationFailed)?;
        Ok(items)
    }

    pub(super) fn filled<T: Copy>(&mut self, len: usize, value: T) -> Result<Vec<T>, E> {
        let mut items = self.reserve(len)?;
        self.price(len)?;
        while items.len() < len {
            self.price(0)?;
            items.resize(items.len() + (len - items.len()).min(POLL_INTERVAL), value);
        }
        Ok(items)
    }

    pub(super) fn fill<T: Clone>(&mut self, len: usize, value: T) -> Result<Vec<T>, E> {
        self.fill_with(len, || value.clone())
    }

    pub(super) fn fill_with<T>(
        &mut self,
        len: usize,
        mut f: impl FnMut() -> T,
    ) -> Result<Vec<T>, E> {
        let mut values = self.reserve(len)?;
        self.price(len)?;
        for i in 0..len {
            self.poll(i)?;
            values.push(f());
        }
        Ok(values)
    }

    pub(super) fn map<T, U>(&mut self, source: &[T], map: impl Fn(&T) -> U) -> Result<Vec<U>, E> {
        let mut items = self.reserve(source.len())?;
        self.price(source.len())?;
        for chunk in source.chunks(POLL_INTERVAL) {
            self.price(0)?;
            items.extend(chunk.iter().map(&map));
        }
        Ok(items)
    }

    pub(super) fn copy<T: Copy>(&mut self, source: &[T]) -> Result<Vec<T>, E> {
        self.map(source, |item| *item)
    }

    pub(super) fn copy_into<T: Copy>(&mut self, items: &mut [T], source: &[T]) -> Result<(), E> {
        self.price(source.len())?;
        for (target, source) in items
            .chunks_mut(POLL_INTERVAL)
            .zip(source.chunks(POLL_INTERVAL))
        {
            self.price(0)?;
            target.copy_from_slice(source);
        }
        Ok(())
    }

    pub(super) fn indices(&mut self, len: usize) -> Result<Vec<usize>, E> {
        let mut items = self.reserve(len)?;
        self.price(len)?;
        for i in 0..len {
            self.poll(i)?;
            items.push(i);
        }
        Ok(items)
    }

    pub(super) fn push<T>(&mut self, items: &mut Vec<T>, value: T) -> Result<(), E> {
        self.price(1)?;
        if items.len() == items.capacity() {
            self.price(items.len())?;
            items
                .try_reserve(1)
                .map_err(|_| CompoundError::AllocationFailed)?;
        }
        items.push(value);
        Ok(())
    }

    pub(super) fn finite(&mut self, xs: &[f64]) -> Result<(), E> {
        self.price(xs.len())?;
        for (i, &x) in xs.iter().enumerate() {
            self.poll(i)?;
            if !x.is_finite() {
                return Err(CompoundError::NonFiniteGeometry);
            }
        }
        Ok(())
    }

    pub(super) fn sort(
        &mut self,
        ids: &mut [usize],
        compare: impl Fn(usize, usize) -> Ordering,
    ) -> Result<(), E> {
        self.sort_with(ids, &mut Vec::new(), |a, b, _| Ok(compare(a, b)))
    }

    pub(super) fn sort_rows<T: Copy>(
        &mut self,
        rows: &mut [T],
        compare: impl Fn(&T, &T) -> Ordering,
    ) -> Result<(), E> {
        self.sort_with(rows, &mut Vec::new(), |a, b, _| Ok(compare(&a, &b)))
    }

    pub(super) fn solve(
        &mut self,
        solve: &mut Solve<'_, E>,
        plan: LevelPlan<'_>,
        mode: Mode,
    ) -> Result<LevelResult, E> {
        self.price(0)?;
        let result = solve(plan, mode, self.charge);
        self.price(0)?;
        result
    }

    /// Stable ordering, also for JavaScript numeric ties such as signed zero.
    /// Comparators that walk a group chain price each hop through this same meter.
    pub(super) fn sort_with<T: Copy>(
        &mut self,
        ids: &mut [T],
        spare: &mut Vec<T>,
        mut compare: impl FnMut(T, T, &mut Self) -> Result<Ordering, E>,
    ) -> Result<(), E> {
        const RUN: usize = 8;
        let len = ids.len();
        if len < 2 {
            return Ok(());
        }
        let depth = (len - 1).ilog2() as usize + 1;
        self.scaled(
            len,
            len.min(RUN) + depth.saturating_sub(RUN.ilog2() as usize) + 1,
        )?;
        for (i, run) in ids.chunks_mut(RUN).enumerate() {
            self.poll(i * RUN)?;
            for index in 1..run.len() {
                let item = run[index];
                let mut dest = index;
                while dest > 0 && compare(item, run[dest - 1], self)?.is_lt() {
                    run[dest] = run[dest - 1];
                    dest -= 1;
                }
                run[dest] = item;
            }
        }
        if len <= RUN {
            return Ok(());
        }
        if spare.len() < len {
            let seed = ids[0];
            self.price(len - spare.len())?;
            if spare.capacity() < len {
                self.price(spare.len())?;
                spare
                    .try_reserve(len - spare.len())
                    .map_err(|_| CompoundError::AllocationFailed)?;
            }
            while spare.len() < len {
                self.price(0)?;
                spare.resize(spare.len() + (len - spare.len()).min(POLL_INTERVAL), seed);
            }
        }
        let mut from = ids;
        let mut to = &mut spare[..len];
        let mut width = RUN;
        let mut in_input = true;
        while width < len {
            let span = add(width, width)?;
            for (source, output) in from.chunks(span).zip(to.chunks_mut(span)) {
                let (left, right) = source.split_at(width.min(source.len()));
                let (mut l, mut r) = (0, 0);
                for (i, dest) in output.iter_mut().enumerate() {
                    self.poll(i)?;
                    if r < right.len()
                        && (l == left.len() || compare(right[r], left[l], self)?.is_lt())
                    {
                        *dest = right[r];
                        r += 1;
                    } else {
                        *dest = left[l];
                        l += 1;
                    }
                }
            }
            std::mem::swap(&mut from, &mut to);
            in_input = !in_input;
            width = span;
        }
        if !in_input {
            self.copy_into(to, from)?;
        }
        Ok(())
    }
}

pub(super) fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b || (a == b && a.is_sign_negative()) {
        a
    } else {
        b
    }
}

pub(super) fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b || (a == b && b.is_sign_negative()) {
        a
    } else {
        b
    }
}

pub(super) fn compare_number(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

pub(super) use compare_number as cmp;
