use super::*;

/// An occluded horizontal terminal can detour into another terminal's entry lane.
/// Preserve its terminal side and try an open perimeter of the same group.
pub(super) fn prefer_terminal_perimeter(
    context: &Routing<'_>,
    plan: &mut Plan,
    group_sites: &[usize],
    routers: &mut Vec<Option<OrthoRouter>>,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    let hierarchy = context.hierarchy;
    let separation = context.metrics.separation;
    let mut face_uses = Vec::new();
    let mut changed = false;
    for path in &plan.paths {
        work.visit(1)?;
        let [first, _, .., last] = path.as_slice() else {
            continue;
        };
        let first = plan.scopes[first.scope][first.index];
        let last = plan.scopes[last.scope][last.index];
        let terminals = [plan.sites[first.from.site], plan.sites[last.to.site]];
        if terminals[0].scope == terminals[1].scope
            || terminals
                .iter()
                .any(|site| site.owner >= hierarchy.node_count || site.scope == hierarchy.root())
        {
            continue;
        }
        for (site, border) in terminals.into_iter().zip([first.to.site, last.from.site]) {
            work.visit(1)?;
            let portal = plan.sites[border];
            let scope = site.scope;
            if portal.owner != hierarchy.node_count + scope
                || portal.side != site.side
                || site.side.axis() != Axis::Horizontal
                || hierarchy.scopes[scope].items.len() < 2
            {
                continue;
            }
            let frame = context.bounds(scope);
            let bounds = Bounds::of(frame);
            let (lead, _) = context.lead(site, false);
            let (_, sign) = normal(site.side);
            let face = bounds.x[usize::from(sign > 0)];
            if (face - lead.x) * sign as f64 <= 0.0 {
                continue;
            }
            let border_point = knot(Point { x: face, y: lead.y }, Axis::Horizontal);
            let Some(straight) = OrthoSegment::between(lead, border_point) else {
                continue;
            };
            if face_uses.is_empty() {
                work.charge(context.graph.groups.len())?;
                face_uses = filled(context.graph.groups.len(), [0usize; 4])?;
                for &index in group_sites {
                    work.visit(1)?;
                    let site = plan.sites[index];
                    face_uses[site.owner - hierarchy.node_count][site.side as usize] += 1;
                }
            }
            let uses = &mut face_uses[scope];
            if uses[portal.side as usize] < 2 {
                continue;
            }
            if routers.is_empty() {
                work.charge(hierarchy.scopes.len())?;
                *routers = reserved(hierarchy.scopes.len())?;
                routers.resize_with(hierarchy.scopes.len(), || None);
            }
            let router = scope_router(routers, context, scope, work)?;
            if router.channel(straight, Some(frame), work)?.is_some() {
                continue;
            }
            let parent = Bounds::of(context.bounds(portal.scope));
            let nearer = if lead.y <= frame.center().y {
                Side::Top
            } else {
                Side::Bottom
            };
            for side in [nearer, opposite(nearer)] {
                // Keep the alternate contact at the checked coordinate when ports are seated.
                if uses[side as usize] != 0
                    || context.border_title_gap(portal.owner, side, true).is_some()
                {
                    continue;
                }
                let [lo, _, hi] = context.seat(portal.owner, side, true);
                let (_, sign) = normal(side);
                let y = bounds.y[usize::from(sign > 0)];
                if !(lo..=hi).contains(&lead.x) || (y - lead.y) * sign as f64 <= separation {
                    continue;
                }
                let contact = Point { x: lead.x, y };
                let Some(perimeter) = OrthoSegment::between(lead, knot(contact, Axis::Vertical))
                else {
                    continue;
                };
                if scope_router(routers, context, scope, work)?
                    .channel(perimeter, Some(frame), work)?
                    .is_none()
                {
                    continue;
                }
                let outside = Point {
                    y: y + sign as f64 * separation,
                    ..contact
                };
                if !scope_router(routers, context, portal.scope, work)?.area_is_clear(
                    Bounds::between(contact, outside),
                    parent,
                    Some(frame),
                    work,
                )? {
                    continue;
                }
                let portal = &mut plan.sites[border];
                uses[portal.side as usize] -= 1;
                uses[side as usize] += 1;
                portal.side = side;
                portal.preferred = lead.x;
                portal.tie_preferred = lead.x;
                changed = true;
                break;
            }
        }
    }
    Ok(changed)
}

fn scope_router<'a>(
    routers: &'a mut [Option<OrthoRouter>],
    context: &Routing<'_>,
    scope: usize,
    work: &mut Work<'_>,
) -> LayoutResult<&'a OrthoRouter> {
    let entry = &mut routers[scope];
    if entry.is_none() {
        *entry = Some(context.router(scope, work)?);
    }
    Ok(entry.as_ref().expect("initialized scope router"))
}
