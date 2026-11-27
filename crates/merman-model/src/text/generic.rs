//! Mermaid generic-type canonicalization shared by source and output consumers.

use std::ops::Range;

use super::is_ecmascript_whitespace;

pub fn count_occurrence(haystack: &str, needle: char) -> usize {
    haystack.chars().filter(|c| *c == needle).count()
}

fn should_combine_sets(previous_tilde_count: usize, next_tilde_count: usize) -> bool {
    previous_tilde_count == 1 && next_tilde_count == 1
}

#[derive(Debug, Clone, Copy)]
enum GenericTypeSet<'a> {
    Single { value: &'a str, tilde_count: usize },
    Combined { left: &'a str, right: &'a str },
}

impl<'a> GenericTypeSet<'a> {
    fn visit<E>(
        self,
        angles: [&'a str; 2],
        visit: &mut impl FnMut(&'a str) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Single { value, tilde_count } => {
                visit_processed_generic_set(&[value], tilde_count, angles, visit)
            }
            Self::Combined { left, right } => {
                visit_processed_generic_set(&[left, ",", right], 2, angles, visit)
            }
        }
    }
}

fn visit_processed_generic_set<'a, E>(
    parts: &[&'a str],
    tilde_count: usize,
    [open, close]: [&'a str; 2],
    visit: &mut impl FnMut(&'a str) -> Result<(), E>,
) -> Result<(), E> {
    if tilde_count <= 1 {
        for part in parts {
            visit(part)?;
        }
        return Ok(());
    }

    let preserve_leading_tilde =
        !tilde_count.is_multiple_of(2) && parts.first().is_some_and(|part| part.starts_with('~'));
    let paired_count = tilde_count / 2;
    let mut tilde_index = 0usize;
    for part in parts {
        let mut retained_start = 0usize;
        for (offset, ch) in part.char_indices() {
            if ch != '~' {
                continue;
            }
            if retained_start < offset {
                visit(&part[retained_start..offset])?;
            }
            let replacement = if preserve_leading_tilde && tilde_index == 0 {
                "~"
            } else {
                let paired_index = tilde_index - usize::from(preserve_leading_tilde);
                if paired_index < paired_count {
                    open
                } else if !preserve_leading_tilde
                    && !tilde_count.is_multiple_of(2)
                    && paired_index == paired_count
                {
                    "~"
                } else {
                    close
                }
            };
            visit(replacement)?;
            retained_start = offset + ch.len_utf8();
            tilde_index += 1;
        }
        if retained_start < part.len() {
            visit(&part[retained_start..])?;
        }
    }
    Ok(())
}

fn visit_legacy_generic_types<'a, E>(
    input: &'a str,
    angles: [&'a str; 2],
    visit: &mut impl FnMut(&'a str) -> Result<(), E>,
) -> Result<(), E> {
    let mut sets = input.split(',');
    let first = sets.next().unwrap_or_default();
    let mut previous = first;
    let mut previous_tilde_count = count_occurrence(first, '~');
    let mut pending = GenericTypeSet::Single {
        value: first,
        tilde_count: previous_tilde_count,
    };

    for next in sets {
        let next_tilde_count = count_occurrence(next, '~');
        if should_combine_sets(previous_tilde_count, next_tilde_count) {
            // Mermaid removes the previously emitted set before combining the two raw sets around
            // the comma. Keep the last set pending so that this replacement stays allocation-free.
            pending = GenericTypeSet::Combined {
                left: previous,
                right: next,
            };
        } else {
            pending.visit(angles, visit)?;
            visit(",")?;
            pending = GenericTypeSet::Single {
                value: next,
                tilde_count: next_tilde_count,
            };
        }
        previous = next;
        previous_tilde_count = next_tilde_count;
    }

    pending.visit(angles, visit)
}

fn type_name_end(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn nested_opening(input: &str, at: usize) -> bool {
    let following = &input[at + 1..];
    // An array suffix attaches to the preceding closed type.
    if following.starts_with("[]") {
        return false;
    }
    input[..at].chars().next_back().is_some_and(type_name_end)
        && following.chars().next().is_some_and(|ch| {
            ch.is_alphanumeric() || matches!(ch, '_' | '?' | '\'' | '(' | '[' | '&')
        })
}

fn closed_group_len(input: &str) -> Option<usize> {
    let body = input.strip_prefix('~')?;
    let mut depth = 1usize;
    for (at, ch) in body.char_indices() {
        match ch {
            '~' if nested_opening(input, at + 1) => depth += 1,
            '~' => {
                depth -= 1;
                if depth == 0 {
                    return Some(at + 2);
                }
            }
            '\r' | '\n' | '\u{2028}' | '\u{2029}' | '{' | '}' => return None,
            _ => {}
        }
    }
    None
}

fn next_closed_group(input: &str, from: usize) -> Result<Option<Range<usize>>, ()> {
    // A leading tilde can be package visibility in a member signature.
    let search = from.max(usize::from(input.starts_with('~')));
    let Some(at) = input[search..].find('~').map(|offset| search + offset) else {
        return Ok(None);
    };
    if !input[from..at]
        .trim_end_matches(is_ecmascript_whitespace)
        .chars()
        .next_back()
        .is_some_and(type_name_end)
    {
        return Err(());
    }
    let len = closed_group_len(&input[at..]).ok_or(())?;
    Ok(Some(at..at + len))
}

fn visit_closed_generic_types<'a, E>(
    input: &'a str,
    [open, close]: [&'a str; 2],
    visit: &mut impl FnMut(&'a str) -> Result<(), E>,
) -> Result<(), E> {
    let mut start = 0;
    while let Some(group) = next_closed_group(input, start).expect("validated generic groups") {
        let mut retained = start;
        for (at, _) in input[group.clone()].match_indices('~') {
            let at = group.start + at;
            if retained < at {
                visit(&input[retained..at])?;
            }
            if at == group.start || nested_opening(input, at) {
                visit(open)?;
            } else {
                visit(close)?;
            }
            retained = at + 1;
        }
        start = group.end;
    }
    visit(&input[start..])
}

#[derive(Debug, Clone, Copy)]
enum GenericSyntax {
    Plain,
    Closed,
    Legacy,
}

/// An allocation-free plan for Mermaid's generic-type canonicalization.
#[derive(Debug, Clone, Copy)]
pub struct GenericTypesPlan<'a> {
    input: &'a str,
    output_len: usize,
    syntax: GenericSyntax,
}

impl<'a> GenericTypesPlan<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut start = 0;
        loop {
            match next_closed_group(input, start) {
                Ok(Some(group)) => start = group.end,
                Ok(None) => {
                    return Self {
                        input,
                        output_len: input.len(),
                        syntax: if start == 0 {
                            GenericSyntax::Plain
                        } else {
                            GenericSyntax::Closed
                        },
                    };
                }
                Err(()) => {
                    // Older Mermaid inputs can rely on recovery for ambiguous or unclosed
                    // tildes. Use the selected upstream projection in that case.
                    let mut output_len = 0;
                    Self::visit_legacy(input, |fragment| output_len += fragment.len());
                    return Self {
                        input,
                        output_len,
                        syntax: GenericSyntax::Legacy,
                    };
                }
            }
        }
    }

    /// Finds the end of a closed type argument list beginning at the opening tilde.
    #[doc(hidden)]
    pub fn closed_group_len(input: &str) -> Option<usize> {
        closed_group_len(input)
    }

    #[doc(hidden)]
    pub fn has_closed_groups(self) -> bool {
        matches!(self.syntax, GenericSyntax::Closed)
    }

    /// Returns the parts of the source outside recognized generic arguments.
    #[doc(hidden)]
    pub fn outside_closed_groups(self) -> impl Iterator<Item = Range<usize>> {
        let mut start = Some(0);
        std::iter::from_fn(move || {
            let from = start.take()?;
            if self.has_closed_groups()
                && let Some(group) = next_closed_group(self.input, from).expect("validated groups")
            {
                start = Some(group.end);
                return Some(from..group.start);
            }
            Some(from..self.input.len())
        })
    }

    /// Visits the selected upstream projection, including its recovery for ambiguous tildes.
    #[doc(hidden)]
    pub fn visit_legacy(input: &'a str, mut visit: impl FnMut(&'a str)) {
        let visited = visit_legacy_generic_types(input, ["<", ">"], &mut |fragment| {
            visit(fragment);
            Ok::<(), std::convert::Infallible>(())
        });
        match visited {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }

    pub const fn output_len(self) -> usize {
        self.output_len
    }

    /// Returns a conservative work bound for revisiting the source during materialization.
    pub fn materialization_scan_work(self) -> Option<usize> {
        match self.syntax {
            GenericSyntax::Plain => Some(0),
            GenericSyntax::Closed | GenericSyntax::Legacy => self.input.len().max(1).checked_mul(2),
        }
    }

    /// Visits the canonical text without allocating an intermediate string.
    /// Visits canonical fragments and stops at the first callback error.
    pub fn try_visit<E>(self, mut visit: impl FnMut(&'a str) -> Result<(), E>) -> Result<(), E> {
        self.try_visit_with_angles(["<", ">"], &mut visit)
    }

    fn try_visit_with_angles<E>(
        self,
        angles: [&'a str; 2],
        visit: &mut impl FnMut(&'a str) -> Result<(), E>,
    ) -> Result<(), E> {
        match self.syntax {
            GenericSyntax::Plain => visit(self.input),
            GenericSyntax::Closed => visit_closed_generic_types(self.input, angles, visit),
            GenericSyntax::Legacy => visit_legacy_generic_types(self.input, angles, visit),
        }
    }

    /// Visits canonical fragments without allocating the resulting string.
    pub fn visit(self, visit: impl FnMut(&'a str)) {
        self.visit_with_angles(["<", ">"], visit);
    }

    /// Visits canonical text with alternate spelling for the generated angle delimiters.
    #[doc(hidden)]
    pub fn visit_with_angles(self, angles: [&'a str; 2], mut visit: impl FnMut(&'a str)) {
        let visited = self.try_visit_with_angles(angles, &mut |fragment| {
            visit(fragment);
            Ok::<(), std::convert::Infallible>(())
        });
        match visited {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }
}

pub fn parse_generic_types(input: &str) -> String {
    let plan = GenericTypesPlan::new(input);
    let mut output = String::with_capacity(plan.output_len());
    plan.visit(|fragment| output.push_str(fragment));
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_generic_types_matches_upstream_examples() {
        let cases = [
            ("test~T~", "test<T>"),
            ("test~Array~Array~string~~~", "test<Array<Array<string>>>"),
            (
                "test~Array~Array~string[]~~~",
                "test<Array<Array<string[]>>>",
            ),
            (
                "test ~Array~Array~string[]~~~",
                "test <Array<Array<string[]>>>",
            ),
            ("~test", "~test"),
            ("~test~T~", "~test<T>"),
            ("Map~K,V~", "Map<K,V>"),
            ("A~,B~,C~", "B<,C>"),
            ("foo~A~B~", "foo<A~B>"),
            ("~foo~A~", "~foo<A>"),
            (",~T~", ",<T>"),
        ];
        for (input, expected) in cases {
            assert_eq!(parse_generic_types(input), expected);

            let plan = GenericTypesPlan::new(input);
            let mut visited = String::new();
            plan.visit(|fragment| visited.push_str(fragment));
            assert_eq!(visited, expected);
            assert_eq!(plan.output_len(), expected.len());
        }
    }

    #[test]
    fn closed_generics_keep_all_arguments_and_adjacent_type_groups() {
        for (input, expected) in [
            (
                "Map~String, List~Int~, Set~Entry~K, V~~~",
                "Map<String, List<Int>, Set<Entry<K, V>>>",
            ),
            (
                "Tuple~(int, string)~ pair, Map~K, V~ table",
                "Tuple<(int, string)> pair, Map<K, V> table",
            ),
            ("Box~List~String~[]~ boxes", "Box<List<String>[]> boxes"),
        ] {
            let plan = GenericTypesPlan::new(input);
            assert_eq!(parse_generic_types(input), expected);
            assert_eq!(plan.output_len(), expected.len());
        }
    }
}
