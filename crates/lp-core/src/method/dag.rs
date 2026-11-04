//! F07: the plan map. Lessons declare `depends_on`; code rejects cycles, finds load-bearing
//! nodes (so F18's Feynman pass knows which lessons to require it on), and renders the graph as
//! Mermaid for the learner to confirm.

use std::collections::{HashMap, HashSet};

use crate::course::Lesson;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DagError {
    #[error("the plan has a cycle")]
    Cycle,
}

/// Topological order of lesson ids, or `Err` if `depends_on` forms a cycle. Unknown dependency
/// ids are ignored (treated as already satisfied), since a dangling id is a content bug for the
/// gate above this, not a graph-shape one.
pub fn topo_order(lessons: &[Lesson]) -> Result<Vec<String>, DagError> {
    let ids: HashSet<&str> = lessons.iter().map(|l| l.id.as_str()).collect();
    let deps: HashMap<&str, Vec<&str>> = lessons
        .iter()
        .map(|l| {
            (
                l.id.as_str(),
                l.depends_on
                    .iter()
                    .map(String::as_str)
                    .filter(|d| ids.contains(d))
                    .collect(),
            )
        })
        .collect();
    let mut order = Vec::with_capacity(lessons.len());
    let mut done: HashSet<&str> = HashSet::new();
    let mut in_progress: HashSet<&str> = HashSet::new();

    fn visit<'a>(
        id: &'a str,
        deps: &HashMap<&'a str, Vec<&'a str>>,
        done: &mut HashSet<&'a str>,
        in_progress: &mut HashSet<&'a str>,
        order: &mut Vec<String>,
    ) -> Result<(), DagError> {
        if done.contains(id) {
            return Ok(());
        }
        if !in_progress.insert(id) {
            return Err(DagError::Cycle);
        }
        for &d in deps.get(id).into_iter().flatten() {
            visit(d, deps, done, in_progress, order)?;
        }
        in_progress.remove(id);
        done.insert(id);
        order.push(id.to_string());
        Ok(())
    }

    for l in lessons {
        visit(
            l.id.as_str(),
            &deps,
            &mut done,
            &mut in_progress,
            &mut order,
        )?;
    }
    Ok(order)
}

pub fn has_cycle(lessons: &[Lesson]) -> bool {
    topo_order(lessons).is_err()
}

/// Lessons with 2 or more dependents: F18 requires a Feynman pass on these (and on goal
/// lessons, which the caller already knows without the graph).
pub fn load_bearing(lessons: &[Lesson]) -> HashSet<String> {
    let mut dependents: HashMap<&str, u32> = HashMap::new();
    for l in lessons {
        for d in &l.depends_on {
            *dependents.entry(d.as_str()).or_default() += 1;
        }
    }
    dependents
        .into_iter()
        .filter(|(_, n)| *n >= 2)
        .map(|(id, _)| id.to_string())
        .collect()
}

/// Mermaid `graph TD` text for the plan review screen: one arrow per dependency edge.
pub fn to_mermaid(lessons: &[Lesson]) -> String {
    let mut out = String::from("graph TD\n");
    for l in lessons {
        out.push_str(&format!("  {}[\"{}\"]\n", l.id, l.title.replace('"', "'")));
    }
    for l in lessons {
        for d in &l.depends_on {
            out.push_str(&format!("  {d} --> {}\n", l.id));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::Lesson;

    fn lesson(id: &str, deps: &[&str]) -> Lesson {
        Lesson {
            id: id.into(),
            title: format!("T {id}"),
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
            ..Lesson::default()
        }
    }

    #[test]
    fn linear_order() {
        let ls = vec![lesson("a", &[]), lesson("b", &["a"]), lesson("c", &["b"])];
        assert_eq!(topo_order(&ls).unwrap(), vec!["a", "b", "c"]);
        assert!(!has_cycle(&ls));
    }

    #[test]
    fn cycle_is_rejected() {
        let ls = vec![lesson("a", &["b"]), lesson("b", &["a"])];
        assert_eq!(topo_order(&ls), Err(DagError::Cycle));
        assert!(has_cycle(&ls));
    }

    #[test]
    fn self_dependency_is_a_cycle() {
        let ls = vec![lesson("a", &["a"])];
        assert!(has_cycle(&ls));
    }

    #[test]
    fn dangling_dependency_is_ignored() {
        let ls = vec![lesson("a", &["ghost"])];
        assert_eq!(topo_order(&ls).unwrap(), vec!["a"]);
    }

    #[test]
    fn load_bearing_needs_two_dependents() {
        let ls = vec![
            lesson("a", &[]),
            lesson("b", &["a"]),
            lesson("c", &["a"]),
            lesson("d", &["b"]),
        ];
        assert_eq!(
            load_bearing(&ls),
            ["a".to_string()].into_iter().collect::<HashSet<_>>()
        );
    }

    #[test]
    fn mermaid_has_a_node_and_an_edge() {
        let ls = vec![lesson("a", &[]), lesson("b", &["a"])];
        let m = to_mermaid(&ls);
        assert!(m.contains("a[\"T a\"]"));
        assert!(m.contains("a --> b"));
    }
}
