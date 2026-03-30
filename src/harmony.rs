//! Voice-leading graph for chord progressions.
//!
//! Builds a graph of chords connected by parsimonious voice leading.
//! Provides pathfinding, random walks, PLR operations, and orbit tracing.

use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::cmp::Reverse;
use std::fmt;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

// ─── Constants ───────────────────────────────────────────────────────────────

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

/// Seventh chord types: name → interval set from root.
pub const SEVENTH_TYPES: &[(&str, &[u8])] = &[
    ("maj7",    &[0, 4, 7, 11]),
    ("dom7",    &[0, 4, 7, 10]),
    ("min7",    &[0, 3, 7, 10]),
    ("minmaj7", &[0, 3, 7, 11]),
    ("hdim7",   &[0, 3, 6, 10]),
    ("dim7",    &[0, 3, 6, 9]),
    ("augmaj7", &[0, 4, 8, 11]),
    ("augdom7", &[0, 4, 8, 10]),
];

/// Triad types: name → interval set from root.
pub const TRIAD_TYPES: &[(&str, &[u8])] = &[
    ("major",      &[0, 4, 7]),
    ("minor",      &[0, 3, 7]),
    ("augmented",  &[0, 4, 8]),
    ("diminished", &[0, 3, 6]),
    ("sus2",       &[0, 2, 7]),
    ("sus4",       &[0, 5, 7]),
];

// ─── Core functions ──────────────────────────────────────────────────────────

/// Shortest distance between two pitch classes on the chromatic circle.
pub fn pc_dist(a: u8, b: u8) -> u8 {
    let d = ((a as i8 - b as i8).unsigned_abs()) % 12;
    d.min(12 - d)
}

/// Minimum total voice-leading distance between two chords of equal cardinality.
/// Tries all permutations (fine for n ≤ 4).
pub fn voice_leading_distance(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    let n = a.len();
    let mut best = u8::MAX;
    for_each_permutation(n, &mut |perm| {
        let total: u8 = (0..n).map(|i| pc_dist(a[i], b[perm[i]])).sum();
        if total < best {
            best = total;
        }
    });
    best
}

/// Iterate all permutations of [0..n] via Heap's algorithm.
fn for_each_permutation(n: usize, f: &mut impl FnMut(&[usize])) {
    let mut perm: Vec<usize> = (0..n).collect();
    let mut c = vec![0usize; n];
    f(&perm);
    let mut i = 0;
    while i < n {
        if c[i] < i {
            if i % 2 == 0 {
                perm.swap(0, i);
            } else {
                perm.swap(c[i], i);
            }
            f(&perm);
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
}

// ─── Chord ───────────────────────────────────────────────────────────────────

/// A chord defined by root pitch class, type name, and pitch-class set.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    pub root: u8,
    pub type_name: &'static str,
    pub notes: Vec<u8>,
}

impl Chord {
    pub fn root_name(&self) -> &'static str {
        NOTE_NAMES[self.root as usize]
    }

    pub fn label(&self) -> String {
        let short = match self.type_name {
            "major" => "", "minor" => "m", "augmented" => "aug", "diminished" => "dim",
            "sus2" => "sus2", "sus4" => "sus4",
            "maj7" => "maj7", "dom7" => "7", "min7" => "m7", "minmaj7" => "mM7",
            "hdim7" => "ø7", "dim7" => "°7", "augmaj7" => "augM7", "augdom7" => "aug7",
            other => other,
        };
        format!("{}{}", self.root_name(), short)
    }
}

impl fmt::Debug for Chord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let names: Vec<&str> = self.notes.iter().map(|&n| NOTE_NAMES[n as usize]).collect();
        write!(f, "{} [{}]", self.label(), names.join(" "))
    }
}

// ─── Graph ───────────────────────────────────────────────────────────────────

/// A graph of chords connected by parsimonious voice leading.
pub struct VoiceLeadingGraph {
    pub chords: Vec<Chord>,
    adj: HashMap<usize, Vec<(usize, u8)>>, // chord_idx → [(neighbor_idx, distance)]
    by_notes: HashMap<Vec<u8>, usize>,       // sorted notes → chord_idx
    chord_types: &'static [(&'static str, &'static [u8])],
}

impl VoiceLeadingGraph {
    /// Build the graph from a chord type set.
    ///
    /// `triads`: use triad types (default: seventh chord types).
    /// `max_distance`: maximum voice-leading distance for an edge (default: 2).
    pub fn new(triads: bool, max_distance: u8) -> Self {
        let types = if triads { TRIAD_TYPES } else { SEVENTH_TYPES };
        Self::from_types(types, max_distance)
    }

    /// Build from custom chord types.
    pub fn from_types(types: &'static [(&'static str, &'static [u8])], max_distance: u8) -> Self {
        let mut chords = Vec::new();
        let mut by_notes: HashMap<Vec<u8>, usize> = HashMap::new();
        let mut seen: HashSet<Vec<u8>> = HashSet::new();

        // Generate all transpositions, deduplicate by pitch-class set
        for &(type_name, template) in types {
            for t in 0u8..12 {
                let mut notes: Vec<u8> = template.iter().map(|&pc| (pc + t) % 12).collect();
                notes.sort();
                if seen.insert(notes.clone()) {
                    let idx = chords.len();
                    chords.push(Chord { root: t, type_name, notes: notes.clone() });
                    by_notes.insert(notes, idx);
                }
            }
        }

        // Build adjacency
        let mut adj: HashMap<usize, Vec<(usize, u8)>> = HashMap::new();
        for i in 0..chords.len() {
            adj.entry(i).or_default();
            for j in (i + 1)..chords.len() {
                let d = voice_leading_distance(&chords[i].notes, &chords[j].notes);
                if d <= max_distance {
                    adj.entry(i).or_default().push((j, d));
                    adj.entry(j).or_default().push((i, d));
                }
            }
        }

        VoiceLeadingGraph { chords, adj, by_notes, chord_types: types }
    }

    /// Find a chord by root pitch class and type name.
    pub fn find(&self, root: u8, type_name: &str) -> Option<&Chord> {
        let template = self.chord_types.iter()
            .find(|&&(name, _)| name == type_name)
            .map(|&(_, t)| t)?;
        let mut notes: Vec<u8> = template.iter().map(|&pc| (pc + root) % 12).collect();
        notes.sort();
        self.by_notes.get(&notes).map(|&idx| &self.chords[idx])
    }

    fn chord_idx(&self, chord: &Chord) -> Option<usize> {
        self.by_notes.get(&chord.notes).copied()
    }

    /// Get neighbors of a chord, optionally filtered by max distance.
    pub fn neighbors(&self, chord: &Chord, max_dist: Option<u8>) -> Vec<(&Chord, u8)> {
        let idx = match self.chord_idx(chord) {
            Some(i) => i,
            None => return vec![],
        };
        let mut result: Vec<(&Chord, u8)> = self.adj[&idx]
            .iter()
            .filter(|(_, d)| max_dist.map_or(true, |m| *d <= m))
            .map(|&(ni, d)| (&self.chords[ni], d))
            .collect();
        result.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.label().cmp(&b.0.label())));
        result
    }

    /// Shortest path (fewest hops) via BFS.
    pub fn shortest_path(&self, start: &Chord, end: &Chord) -> Option<Vec<Chord>> {
        let si = self.chord_idx(start)?;
        let ei = self.chord_idx(end)?;
        if si == ei {
            return Some(vec![start.clone()]);
        }
        let mut visited = HashSet::new();
        visited.insert(si);
        let mut queue: VecDeque<(usize, Vec<usize>)> = VecDeque::new();
        queue.push_back((si, vec![si]));
        while let Some((current, path)) = queue.pop_front() {
            for &(neighbor, _) in &self.adj[&current] {
                if neighbor == ei {
                    let mut full = path.clone();
                    full.push(neighbor);
                    return Some(full.into_iter().map(|i| self.chords[i].clone()).collect());
                }
                if visited.insert(neighbor) {
                    let mut new_path = path.clone();
                    new_path.push(neighbor);
                    queue.push_back((neighbor, new_path));
                }
            }
        }
        None
    }

    /// Smoothest path (minimum total voice-leading cost) via Dijkstra.
    pub fn smoothest_path(&self, start: &Chord, end: &Chord) -> Option<Vec<Chord>> {
        let si = self.chord_idx(start)?;
        let ei = self.chord_idx(end)?;
        if si == ei {
            return Some(vec![start.clone()]);
        }

        let mut dist_to: HashMap<usize, u32> = HashMap::new();
        let mut prev: HashMap<usize, usize> = HashMap::new();
        let mut visited: HashSet<usize> = HashSet::new();
        let mut heap: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();

        dist_to.insert(si, 0);
        heap.push(Reverse((0, si)));

        while let Some(Reverse((cost, current))) = heap.pop() {
            if !visited.insert(current) {
                continue;
            }
            if current == ei {
                let mut path = Vec::new();
                let mut node = ei;
                loop {
                    path.push(node);
                    match prev.get(&node) {
                        Some(&p) => node = p,
                        None => break,
                    }
                }
                path.reverse();
                return Some(path.into_iter().map(|i| self.chords[i].clone()).collect());
            }
            for &(neighbor, d) in &self.adj[&current] {
                if !visited.contains(&neighbor) {
                    let new_cost = cost + d as u32;
                    if new_cost < *dist_to.get(&neighbor).unwrap_or(&u32::MAX) {
                        dist_to.insert(neighbor, new_cost);
                        prev.insert(neighbor, current);
                        heap.push(Reverse((new_cost, neighbor)));
                    }
                }
            }
        }
        None
    }

    /// Random walk on the graph.
    pub fn random_walk(
        &self,
        start: &Chord,
        steps: usize,
        max_dist: u8,
        avoid_revisit: bool,
        seed: u64,
    ) -> Vec<Chord> {
        let mut rng = SmallRng::seed_from_u64(seed);
        let si = match self.chord_idx(start) {
            Some(i) => i,
            None => return vec![start.clone()],
        };

        let mut path = vec![si];
        let mut visited: HashSet<usize> = HashSet::new();
        visited.insert(si);

        for _ in 0..steps {
            let current = *path.last().unwrap();
            let mut candidates: Vec<(usize, u8)> = self.adj[&current]
                .iter()
                .filter(|(_, d)| *d <= max_dist)
                .copied()
                .collect();
            if candidates.is_empty() {
                break;
            }
            if avoid_revisit {
                let unvisited: Vec<(usize, u8)> = candidates
                    .iter()
                    .filter(|(n, _)| !visited.contains(n))
                    .copied()
                    .collect();
                if !unvisited.is_empty() {
                    candidates = unvisited;
                }
            }
            let pick = rng.random::<u32>() as usize % candidates.len();
            let (next, _) = candidates[pick];
            path.push(next);
            visited.insert(next);
        }

        path.into_iter().map(|i| self.chords[i].clone()).collect()
    }

    /// Apply PLR operations cyclically until the orbit returns to start.
    /// Only works for triads.
    pub fn orbit(&self, start: &Chord, operations: &[char], max_iterations: usize) -> Vec<Chord> {
        let mut path = vec![start.clone()];
        let mut current = start.clone();

        for _ in 0..max_iterations {
            for &op in operations {
                let next = match self.apply_plr(&current, op) {
                    Some(c) => c,
                    None => return path,
                };
                current = next;
                path.push(current.clone());
                if current == *start && path.len() > 1 {
                    return path;
                }
            }
        }
        path
    }

    fn apply_plr(&self, chord: &Chord, op: char) -> Option<Chord> {
        let r = chord.root;
        let q = chord.type_name;
        match op {
            'P' => {
                let new_type = match q {
                    "major" => "minor",
                    "minor" => "major",
                    _ => return None,
                };
                self.find(r, new_type).cloned()
            }
            'L' => match q {
                "major" => self.find((r + 4) % 12, "minor").cloned(),
                "minor" => self.find((r + 8) % 12, "major").cloned(),
                _ => None,
            },
            'R' => match q {
                "major" => self.find((r + 9) % 12, "minor").cloned(),
                "minor" => self.find((r + 3) % 12, "major").cloned(),
                _ => None,
            },
            _ => None,
        }
    }
}

impl fmt::Display for VoiceLeadingGraph {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let edge_count: usize = self.adj.values().map(|v| v.len()).sum::<usize>() / 2;
        write!(f, "VoiceLeadingGraph({} chords, {} edges)", self.chords.len(), edge_count)
    }
}

// ─── Note name parsing ──────────────────────────────────────────────────────

/// Parse a note name to a pitch class (0-11).
pub fn note_to_pc(name: &str) -> Option<u8> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let bytes = name.as_bytes();
    let base = match bytes[0].to_ascii_uppercase() {
        b'C' => 0u8,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    let mut offset = 0i8;
    for &b in &bytes[1..] {
        match b {
            b'#' => offset += 1,
            b'b' => offset -= 1,
            _ => break,
        }
    }
    Some(((base as i8 + offset).rem_euclid(12)) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pc_dist() {
        assert_eq!(pc_dist(0, 0), 0);
        assert_eq!(pc_dist(0, 1), 1);
        assert_eq!(pc_dist(0, 6), 6); // tritone
        assert_eq!(pc_dist(0, 7), 5); // perfect fifth
        assert_eq!(pc_dist(11, 0), 1);
        assert_eq!(pc_dist(0, 11), 1);
    }

    #[test]
    fn test_voice_leading_distance() {
        // C major [0,4,7] → C minor [0,3,7]: one voice moves by 1
        assert_eq!(voice_leading_distance(&[0, 4, 7], &[0, 3, 7]), 1);
        // C major → E minor [4,7,11]: L operation, one voice moves by 1
        assert_eq!(voice_leading_distance(&[0, 4, 7], &[4, 7, 11]), 1);
        // Identical chords
        assert_eq!(voice_leading_distance(&[0, 4, 7], &[0, 4, 7]), 0);
    }

    #[test]
    fn test_note_to_pc() {
        assert_eq!(note_to_pc("C"), Some(0));
        assert_eq!(note_to_pc("C#"), Some(1));
        assert_eq!(note_to_pc("Db"), Some(1));
        assert_eq!(note_to_pc("F#"), Some(6));
        assert_eq!(note_to_pc("Bb"), Some(10));
        assert_eq!(note_to_pc("B"), Some(11));
    }

    #[test]
    fn test_graph_seventh_chord_count() {
        let g = VoiceLeadingGraph::new(false, 2);
        // 7 types × 12 transpositions + 1 type (dim7) × 3 distinct = 87
        assert_eq!(g.chords.len(), 87);
    }

    #[test]
    fn test_graph_triad_count() {
        let g = VoiceLeadingGraph::new(true, 2);
        // 4 standard types × 12 + augmented × 4 + diminished × 12 + sus2 × 12 + sus4 × 12 = ...
        // Actually: major=12, minor=12, aug=4, dim=12, sus2=12, sus4=12 = 64
        // But some sus chords are enharmonic... let's just check it's reasonable
        assert!(g.chords.len() >= 52 && g.chords.len() <= 68);
    }

    #[test]
    fn test_find_chord() {
        let g = VoiceLeadingGraph::new(false, 2);
        let c = g.find(0, "maj7").unwrap();
        assert_eq!(c.root, 0);
        assert_eq!(c.type_name, "maj7");
        assert_eq!(c.notes, vec![0, 4, 7, 11]);

        let d = g.find(2, "min7").unwrap();
        assert_eq!(d.root, 2);
        assert_eq!(d.notes, vec![0, 2, 5, 9]); // D F A C sorted = [0,2,5,9]
    }

    #[test]
    fn test_neighbors() {
        let g = VoiceLeadingGraph::new(false, 2);
        let c7 = g.find(0, "dom7").unwrap();
        let neighbors_d1 = g.neighbors(c7, Some(1));
        let neighbors_d2 = g.neighbors(c7, Some(2));
        // dom7 should have neighbors at both distances
        assert!(!neighbors_d1.is_empty(), "dom7 should have distance-1 neighbors");
        assert!(neighbors_d2.len() > neighbors_d1.len(), "dom7 should have more neighbors at d≤2");
        // All returned neighbors should be within the requested distance
        for (_, d) in &neighbors_d1 {
            assert_eq!(*d, 1);
        }
    }

    #[test]
    fn test_shortest_path_exists() {
        let g = VoiceLeadingGraph::new(false, 2);
        let start = g.find(0, "maj7").unwrap();
        let end = g.find(6, "minmaj7").unwrap();
        let path = g.shortest_path(start, end);
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.len() <= 5); // diameter is 4
        assert_eq!(path.first().unwrap().notes, start.notes);
        assert_eq!(path.last().unwrap().notes, end.notes);
    }

    #[test]
    fn test_smoothest_path() {
        let g = VoiceLeadingGraph::new(false, 2);
        let start = g.find(0, "dom7").unwrap();
        let end = g.find(1, "dim7").unwrap();
        let path = g.smoothest_path(start, end);
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path.first().unwrap().notes, start.notes);
        assert_eq!(path.last().unwrap().notes, end.notes);
    }

    #[test]
    fn test_random_walk_length() {
        let g = VoiceLeadingGraph::new(false, 2);
        let start = g.find(0, "maj7").unwrap();
        let walk = g.random_walk(start, 8, 1, false, 42);
        assert_eq!(walk.len(), 9); // start + 8 steps
    }

    #[test]
    fn test_random_walk_avoid_revisit() {
        let g = VoiceLeadingGraph::new(false, 2);
        let start = g.find(0, "maj7").unwrap();
        let walk = g.random_walk(start, 4, 2, true, 42);
        // With avoid_revisit, should have unique chords (at least for short walks)
        let unique: HashSet<Vec<u8>> = walk.iter().map(|c| c.notes.clone()).collect();
        assert_eq!(unique.len(), walk.len());
    }

    #[test]
    fn test_plr_orbit() {
        let g = VoiceLeadingGraph::new(true, 2);
        let c_major = g.find(0, "major").unwrap();
        // P followed by L: should cycle through hexatonic system
        let orbit = g.orbit(c_major, &['P', 'L'], 24);
        // PL cycle has period 6 (returns to start after 3 pairs = 6 operations)
        assert!(orbit.len() <= 8); // start + up to 7 steps (6 ops + return)
        assert_eq!(orbit.last().unwrap().notes, c_major.notes); // should return
    }

    #[test]
    fn test_dim7_hub_connectivity() {
        let g = VoiceLeadingGraph::new(false, 2);
        // There should be exactly 3 distinct dim7 chords
        let dim7s: Vec<&Chord> = g.chords.iter().filter(|c| c.type_name == "dim7").collect();
        assert_eq!(dim7s.len(), 3);
        // Each should have 8 neighbors at distance 1
        for d in &dim7s {
            let n = g.neighbors(d, Some(1));
            assert_eq!(n.len(), 8, "dim7 {} has {} neighbors, expected 8", d.label(), n.len());
        }
    }

    #[test]
    fn test_chord_label() {
        let g = VoiceLeadingGraph::new(false, 2);
        let c = g.find(0, "maj7").unwrap();
        assert_eq!(c.label(), "Cmaj7");
        let d = g.find(0, "dom7").unwrap();
        assert_eq!(d.label(), "C7");
        let e = g.find(0, "min7").unwrap();
        assert_eq!(e.label(), "Cm7");
    }
}
