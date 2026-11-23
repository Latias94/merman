use crate::LayoutError;
use std::cmp::Ordering;

pub(crate) type Result<T, E> = std::result::Result<T, LayoutError<E>>;

pub(crate) fn reserved<T, E>(len: usize) -> Result<Vec<T>, E> {
    let mut vec = Vec::new();
    vec.try_reserve_exact(len)
        .map_err(|_| LayoutError::Overflow)?;
    Ok(vec)
}

pub(crate) fn filled<T: Clone, E>(len: usize, value: T) -> Result<Vec<T>, E> {
    let mut vec = reserved(len)?;
    vec.resize(len, value);
    Ok(vec)
}

pub(crate) fn push<T, E>(vec: &mut Vec<T>, value: T) -> Result<(), E> {
    vec.try_reserve(1).map_err(|_| LayoutError::Overflow)?;
    vec.push(value);
    Ok(())
}

pub(crate) struct Work<'a, E> {
    callback: &'a mut dyn FnMut(usize) -> std::result::Result<(), E>,
    pending: usize,
    total: usize,
}

impl<'a, E> Work<'a, E> {
    pub(crate) fn new(callback: &'a mut dyn FnMut(usize) -> std::result::Result<(), E>) -> Self {
        Self {
            callback,
            pending: 0,
            total: 0,
        }
    }

    /// Admit a bulk allocation or flush the final partial traversal batch.
    pub(crate) fn charge(&mut self, units: usize) -> Result<(), E> {
        let units = self
            .pending
            .checked_add(units)
            .ok_or(LayoutError::Overflow)?;
        if units > 0 {
            self.total = self.total.checked_add(units).ok_or(LayoutError::Overflow)?;
            (self.callback)(units).map_err(LayoutError::Interrupted)?;
            self.pending = 0;
        }
        Ok(())
    }

    pub(crate) fn visit(&mut self, units: usize) -> Result<(), E> {
        self.pending = self
            .pending
            .checked_add(units)
            .ok_or(LayoutError::Overflow)?;
        if self.pending >= 128 {
            self.charge(0)?;
        }
        Ok(())
    }

    /// Sort bounded runs with the standard library. The merge of a very wide rank or fan
    /// polls the same control as the graph walks, even after its sort budget was admitted.
    pub(crate) fn sort(
        &mut self,
        ids: &mut [usize],
        cmp: impl Fn(&usize, &usize) -> Ordering,
    ) -> Result<(), E> {
        const RUN: usize = 1024;
        let mut ordered = true;
        for pair in ids.windows(2) {
            self.visit(1)?;
            if cmp(&pair[0], &pair[1]).is_gt() {
                ordered = false;
                break;
            }
        }
        if ordered {
            return Ok(());
        }
        for run in ids.chunks_mut(RUN) {
            let levels = run.len().ilog2() as usize + 1;
            self.visit(run.len() * levels)?;
            run.sort_unstable_by(&cmp);
        }
        if ids.len() <= RUN {
            return Ok(());
        }
        let mut merged = Vec::new();
        let mut width = RUN;
        while width < ids.len() {
            let pair = width.checked_mul(2).ok_or(LayoutError::Overflow)?;
            for block in ids.chunks_mut(pair) {
                self.visit(1)?;
                if block.len() <= width || cmp(&block[width - 1], &block[width]).is_le() {
                    continue;
                }
                merged.clear();
                if merged.capacity() < block.len() {
                    self.charge(block.len())?;
                    merged
                        .try_reserve_exact(block.len())
                        .map_err(|_| LayoutError::Overflow)?;
                }
                let (mut left, mut right) = block.split_at(width);
                while let ([a, rest_a @ ..], [b, rest_b @ ..]) = (left, right) {
                    self.visit(1)?;
                    if cmp(a, b).is_le() {
                        merged.push(*a);
                        left = rest_a;
                    } else {
                        merged.push(*b);
                        right = rest_b;
                    }
                }
                // Only the prefix consumed by the merge needs copying if the right tail
                // already occupies its final position.
                for tail in left.chunks(RUN) {
                    self.visit(tail.len())?;
                    merged.extend_from_slice(tail);
                }
                for (dest, src) in block.chunks_mut(RUN).zip(merged.chunks(RUN)) {
                    self.visit(src.len())?;
                    dest[..src.len()].copy_from_slice(src);
                }
            }
            width = pair;
        }
        Ok(())
    }
}
