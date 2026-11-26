use super::{Expanded, Place, numeric_cmp};
use crate::work::{LayoutResult, Work, filled, push};
use crate::{GroupId, LayoutError, WorkError};

const UNSEEN: usize = usize::MAX;
const PENDING: usize = usize::MAX - 1;

fn depths(parents: &[Option<GroupId>], work: &mut Work<'_>) -> LayoutResult<Vec<usize>> {
    work.charge(parents.len())?;
    let mut depth = filled(parents.len(), UNSEEN)?;
    let mut path = Vec::new();
    for group in 0..parents.len() {
        let mut current = Some(GroupId(group as u32));
        let mut base = 0;
        while let Some(GroupId(id)) = current {
            work.visit(1)?;
            let id = id as usize;
            let slot = depth
                .get_mut(id)
                .ok_or(LayoutError::InvalidInput("missing order group"))?;
            match *slot {
                PENDING => return Err(LayoutError::InvalidInput("cyclic order groups")),
                UNSEEN => {
                    *slot = PENDING;
                    push(&mut path, id)?;
                    current = parents[id];
                }
                known => {
                    base = known;
                    break;
                }
            }
        }
        while let Some(id) = path.pop() {
            work.visit(1)?;
            base += 1;
            depth[id] = base;
        }
    }
    Ok(depth)
}

#[derive(Clone, Copy)]
enum Item {
    Vertex(u32),
    Group(usize),
}

#[derive(Clone, Copy, Default)]
struct Mean {
    sum: f64,
    count: usize,
    first: usize,
}

impl Mean {
    fn include(&mut self, other: Self) {
        if self.count == 0 || other.first < self.first {
            self.first = other.first;
        }
        self.sum += other.sum;
        self.count += other.count;
    }
}

pub(super) struct Groups<'a> {
    pub(super) owner: Vec<Option<GroupId>>,
    parents: &'a [Option<GroupId>],
    active: Vec<usize>,
    touched: Vec<usize>,
    path: Vec<usize>,
    children: Vec<Vec<Item>>,
    means: Vec<Mean>,
    stack: Vec<Item>,
}

impl<'a> Groups<'a> {
    pub(super) fn new(
        g: &Expanded,
        groups: &[Option<GroupId>],
        parents: &'a [Option<GroupId>],
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        u32::try_from(parents.len()).map_err(|_| WorkError::ArithmeticOverflow)?;
        let depth = depths(parents, work)?;
        work.charge(g.rank.len())?;
        let mut owner = filled(g.rank.len(), None)?;
        for (out, &group) in owner.iter_mut().zip(groups) {
            work.visit(1)?;
            if group.is_some_and(|group| group.0 as usize >= parents.len()) {
                return Err(LayoutError::InvalidInput("missing vertex order group"));
            }
            *out = group;
        }
        for chain in &g.chains {
            work.visit(1)?;
            if chain.len() < 3 {
                continue;
            }
            let mut left = owner[chain[0] as usize];
            let mut right = owner[*chain.last().unwrap() as usize];
            while let (Some(a), Some(b)) = (left, right) {
                if a == b {
                    break;
                }
                work.visit(1)?;
                let (a, b) = (a.0 as usize, b.0 as usize);
                if depth[a] >= depth[b] {
                    left = parents[a];
                }
                if depth[b] >= depth[a] {
                    right = parents[b];
                }
            }
            let common = left.and(right);
            for &vertex in &chain[1..chain.len() - 1] {
                work.visit(1)?;
                owner[vertex as usize] = common;
            }
        }
        work.charge(parents.len())?;
        Ok(Self {
            owner,
            parents,
            active: filled(parents.len(), UNSEEN)?,
            touched: Vec::new(),
            path: Vec::new(),
            children: Vec::new(),
            means: Vec::new(),
            stack: Vec::new(),
        })
    }

    fn block(&mut self, mut group: Option<GroupId>, work: &mut Work<'_>) -> LayoutResult<usize> {
        let mut parent = 0;
        while let Some(GroupId(id)) = group {
            work.visit(1)?;
            let id = id as usize;
            if self.active[id] != UNSEEN {
                parent = self.active[id];
                break;
            }
            push(&mut self.path, id)?;
            group = self.parents[id];
        }
        while let Some(id) = self.path.pop() {
            work.visit(1)?;
            let block = self.means.len();
            if block == self.children.len() {
                push(&mut self.children, Vec::new())?;
            }
            push(&mut self.means, Mean::default())?;
            self.active[id] = block;
            push(&mut self.touched, id)?;
            push(&mut self.children[parent], Item::Group(block))?;
            parent = block;
        }
        Ok(parent)
    }

    pub(super) fn arrange(
        &mut self,
        row: &mut [u32],
        place: &[Place],
        key: impl Fn(usize) -> f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        if row.len() < 2 {
            return Ok(false);
        }
        if self.children.is_empty() {
            push(&mut self.children, Vec::new())?;
        }
        push(&mut self.means, Mean::default())?;
        for &v in row.iter() {
            work.visit(1)?;
            let block = self.block(self.owner[v as usize], work)?;
            push(&mut self.children[block], Item::Vertex(v))?;
        }
        for block in (0..self.means.len()).rev() {
            let children = &mut self.children[block];
            let summary = |item: Item| match item {
                Item::Group(index) => self.means[index],
                Item::Vertex(id) => {
                    let v = id as usize;
                    Mean {
                        sum: key(v),
                        count: 1,
                        first: place[v].pos,
                    }
                }
            };
            let mut total = Mean::default();
            for &child in children.iter() {
                work.visit(1)?;
                total.include(summary(child));
            }
            if !total.sum.is_finite() {
                return Err(WorkError::ArithmeticOverflow.into());
            }
            work.charge_sort(children.len())?;
            children.sort_unstable_by(|&a, &b| {
                let (a, b) = (summary(a), summary(b));
                numeric_cmp(a.sum / a.count as f64, b.sum / b.count as f64)
                    .then(a.first.cmp(&b.first))
            });
            work.check()?;
            self.means[block] = total;
        }
        push(&mut self.stack, Item::Group(0))?;
        let mut at = row.iter_mut();
        let mut changed = false;
        while let Some(item) = self.stack.pop() {
            work.visit(1)?;
            match item {
                Item::Vertex(v) => {
                    let slot = at.next().unwrap();
                    changed |= *slot != v;
                    *slot = v;
                }
                Item::Group(block) => {
                    work.visit(self.children[block].len())?;
                    for child in self.children[block].drain(..).rev() {
                        push(&mut self.stack, child)?;
                    }
                }
            }
        }
        self.means.clear();
        work.visit(self.touched.len())?;
        for group in self.touched.drain(..) {
            self.active[group] = UNSEEN;
        }
        Ok(changed)
    }
}
