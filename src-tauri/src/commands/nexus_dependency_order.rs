//! Stable dependency order. Cycles remain grouped in their original order.
fn visit(node: usize, graph: &[Vec<usize>], seen: &mut [bool], result: &mut Vec<usize>) {
    if seen[node] { return; }
    seen[node] = true;
    for &next in &graph[node] { visit(next, graph, seen, result); }
    result.push(node);
}
pub(super) fn order(dependencies: &[Vec<usize>]) -> Vec<usize> {
    let count = dependencies.len();
    let mut seen = vec![false; count];
    let mut finished = Vec::new();
    for node in 0..count { visit(node, dependencies, &mut seen, &mut finished); }
    let mut reverse = vec![Vec::new(); count];
    for (node, deps) in dependencies.iter().enumerate() {
        for &dep in deps { reverse[dep].push(node); }
    }
    seen.fill(false);
    let mut groups = Vec::new();
    let mut membership = vec![0; count];
    for node in finished.into_iter().rev() {
        if seen[node] { continue; }
        let mut members = Vec::new();
        visit(node, &reverse, &mut seen, &mut members);
        members.sort_unstable();
        for &member in &members { membership[member] = groups.len(); }
        groups.push(members);
    }
    let mut remaining = vec![true; groups.len()];
    let mut result = Vec::with_capacity(count);
    while result.len() < count {
        let ready = (0..groups.len()).filter(|&group| remaining[group] && groups[group].iter().all(|&node|
            dependencies[node].iter().all(|&dep| membership[dep] == group || !remaining[membership[dep]])
        )).min_by_key(|&group| groups[group][0]).expect("condensation graph is acyclic");
        remaining[ready] = false;
        result.extend_from_slice(&groups[ready]);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::order;
    #[test] fn dependencies_before_dependents() { assert_eq!(order(&[vec![1], vec![2], vec![]]), vec![2,1,0]); }
    #[test] fn unrelated_mods_keep_order() { assert_eq!(order(&[vec![], vec![3], vec![], vec![]]), vec![0,2,3,1]); }
    #[test] fn cycles_keep_members_and_release_dependents() { assert_eq!(order(&[vec![1], vec![2], vec![1], vec![]]), vec![1,2,0,3]); }
    #[test] fn duplicates_and_self_dependencies_are_safe() { assert_eq!(order(&[vec![1,1], vec![1]]), vec![1,0]); }
    #[test] fn empty_and_independent_lists() { assert!(order(&[]).is_empty()); assert_eq!(order(&[vec![],vec![]]),vec![0,1]); }
}
