use std::collections::{BinaryHeap, HashMap};

use openmmo_common::TilePos;

#[derive(Debug, Clone, Eq, PartialEq)]
struct Node {
    pos: TilePos,
    g: i32,
    f: i32,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.f.cmp(&self.f)
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub fn find_path(
    start: TilePos,
    goal: TilePos,
    walkable: &std::collections::HashSet<TilePos>,
) -> Vec<TilePos> {
    if start == goal {
        return vec![goal];
    }
    if !walkable.contains(&goal) {
        return Vec::new();
    }

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<TilePos, TilePos> = HashMap::new();
    let mut g_score: HashMap<TilePos, i32> = HashMap::new();

    g_score.insert(start, 0);
    open.push(Node {
        pos: start,
        g: 0,
        f: heuristic(start, goal),
    });

    let directions = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];

    while let Some(current) = open.pop() {
        if current.pos == goal {
            return reconstruct(&came_from, goal);
        }

        for (dx, dy) in directions {
            let neighbor = TilePos::new(current.pos.x + dx, current.pos.y + dy);
            if !is_step_walkable(current.pos, dx, dy, walkable) {
                continue;
            }
            let tentative = current.g + 1;
            let entry = g_score.entry(neighbor).or_insert(i32::MAX);
            if tentative < *entry {
                *entry = tentative;
                came_from.insert(neighbor, current.pos);
                open.push(Node {
                    pos: neighbor,
                    g: tentative,
                    f: tentative + heuristic(neighbor, goal),
                });
            }
        }
    }
    Vec::new()
}

fn is_step_walkable(
    from: TilePos,
    dx: i32,
    dy: i32,
    walkable: &std::collections::HashSet<TilePos>,
) -> bool {
    let to = TilePos::new(from.x + dx, from.y + dy);
    if !walkable.contains(&to) {
        return false;
    }
    if dx != 0 && dy != 0 {
        walkable.contains(&TilePos::new(from.x + dx, from.y))
            && walkable.contains(&TilePos::new(from.x, from.y + dy))
    } else {
        true
    }
}

fn heuristic(a: TilePos, b: TilePos) -> i32 {
    a.chebyshev_distance(&b)
}

const ADJACENT_DIRECTIONS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// Shortest path from `start` to a walkable tile adjacent to `target`.
pub fn find_attack_path(
    start: TilePos,
    target: TilePos,
    walkable: &std::collections::HashSet<TilePos>,
) -> Vec<TilePos> {
    if start.chebyshev_distance(&target) <= 1 {
        return vec![start];
    }

    let mut best_path = Vec::new();
    for (dx, dy) in ADJACENT_DIRECTIONS {
        let adj = TilePos::new(target.x + dx, target.y + dy);
        if !walkable.contains(&adj) {
            continue;
        }
        let path = find_path(start, adj, walkable);
        if path.len() > 1 && (best_path.is_empty() || path.len() < best_path.len()) {
            best_path = path;
        }
    }
    best_path
}

fn reconstruct(came_from: &HashMap<TilePos, TilePos>, mut current: TilePos) -> Vec<TilePos> {
    let mut path = vec![current];
    while let Some(&prev) = came_from.get(&current) {
        current = prev;
        path.push(current);
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn finds_simple_path() {
        let mut walkable = HashSet::new();
        for x in 0..10 {
            for y in 0..10 {
                walkable.insert(TilePos::new(x, y));
            }
        }
        let path = find_path(TilePos::new(0, 0), TilePos::new(3, 3), &walkable);
        assert!(!path.is_empty());
        assert_eq!(path[0], TilePos::new(0, 0));
        assert_eq!(*path.last().unwrap(), TilePos::new(3, 3));
    }

    #[test]
    fn prefers_diagonal_path_when_open() {
        let mut walkable = HashSet::new();
        for x in 0..10 {
            for y in 0..10 {
                walkable.insert(TilePos::new(x, y));
            }
        }
        let path = find_path(TilePos::new(0, 0), TilePos::new(3, 3), &walkable);
        assert_eq!(path.len(), 4);
        assert_eq!(path[1], TilePos::new(1, 1));
        assert_eq!(path[2], TilePos::new(2, 2));
    }

    #[test]
    fn finds_path_to_adjacent_tile() {
        let mut walkable = HashSet::new();
        for x in 0..10 {
            for y in 0..10 {
                walkable.insert(TilePos::new(x, y));
            }
        }
        let target = TilePos::new(5, 5);
        walkable.remove(&target);
        let path = find_attack_path(TilePos::new(0, 0), target, &walkable);
        assert!(!path.is_empty());
        assert_eq!(path[0], TilePos::new(0, 0));
        assert_eq!(path.last().unwrap().chebyshev_distance(&target), 1);
    }

    #[test]
    fn blocks_corner_cutting() {
        let mut walkable = HashSet::new();
        for x in 0..5 {
            for y in 0..5 {
                walkable.insert(TilePos::new(x, y));
            }
        }
        walkable.remove(&TilePos::new(1, 0));
        walkable.remove(&TilePos::new(0, 1));
        let path = find_path(TilePos::new(0, 0), TilePos::new(1, 1), &walkable);
        assert!(path.is_empty());
    }
}
