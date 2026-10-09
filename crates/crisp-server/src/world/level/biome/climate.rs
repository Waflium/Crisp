use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;

use crate::world::level::levelgen::density_function::float;

pub fn quantize_coord(coord: f32) -> i64 {
    (coord * 10000.0) as i64
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Parameter {
    pub min: i64,
    pub max: i64,
}

impl Parameter {
    pub fn distance(&self, target: i64) -> i64 {
        let above = target - self.max;
        let below = self.min - target;
        if above > 0 { above } else { below.max(0) }
    }

    fn span(&self, other: Option<Parameter>) -> Parameter {
        match other {
            None => *self,
            Some(o) => Parameter { min: self.min.min(o.min), max: self.max.max(o.max) },
        }
    }

    pub fn parse(v: &Value) -> Result<Parameter, String> {
        Self::parse_with(v, |v| float(v).map(quantize_coord))
    }

    pub fn parse_report(v: &Value) -> Result<Parameter, String> {
        Self::parse_with(v, |v| v.as_f64().map(|f| (f * 10000.0).round() as i64).ok_or_else(|| format!("bad number {v}")))
    }

    fn parse_with(v: &Value, q: impl Fn(&Value) -> Result<i64, String>) -> Result<Parameter, String> {
        let (min, max) = match v {
            Value::Array(a) if a.len() == 2 => (q(&a[0])?, q(&a[1])?),
            Value::Object(_) => (q(v.get("min").ok_or("min")?)?, q(v.get("max").ok_or("max")?)?),
            _ => (q(v)?, q(v)?),
        };
        if min > max {
            return Err(format!("min > max: {min} {max}"));
        }
        Ok(Parameter { min, max })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParameterPoint {
    pub space: [Parameter; 7],
}

impl ParameterPoint {
    pub fn parse_report(v: &Value) -> Result<ParameterPoint, String> {
        let p = |key: &str| Parameter::parse_report(v.get(key).ok_or_else(|| format!("missing {key}"))?);
        let offset = Parameter::parse_report(v.get("offset").ok_or("missing offset")?)?.min;
        Ok(ParameterPoint {
            space: [
                p("temperature")?,
                p("humidity")?,
                p("continentalness")?,
                p("erosion")?,
                p("depth")?,
                p("weirdness")?,
                Parameter { min: offset, max: offset },
            ],
        })
    }
}

pub type TargetPoint = [i64; 7];

pub fn target(temperature: f32, humidity: f32, continentalness: f32, erosion: f32, depth: f32, weirdness: f32) -> TargetPoint {
    [
        quantize_coord(temperature),
        quantize_coord(humidity),
        quantize_coord(continentalness),
        quantize_coord(erosion),
        quantize_coord(depth),
        quantize_coord(weirdness),
        0,
    ]
}

enum Node {
    Leaf(usize),
    SubTree { space: [Parameter; 7], children: Vec<Node> },
}

pub struct ParameterList<T> {
    values: Vec<(ParameterPoint, T)>,
    root: Node,
    id: usize,
}

static NEXT_TREE_ID: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static LAST_RESULT: RefCell<HashMap<usize, usize>> = RefCell::new(HashMap::new());
}

fn center(p: Parameter) -> i64 {
    (p.min + p.max) / 2
}

fn cost(space: &[Parameter; 7]) -> i64 {
    space.iter().map(|p| (p.max - p.min).abs()).sum()
}

impl<T> ParameterList<T> {
    pub fn new(values: Vec<(ParameterPoint, T)>) -> ParameterList<T> {
        assert!(!values.is_empty(), "Need at least one value to build the search tree.");
        let leaves = (0..values.len()).map(Node::Leaf).collect();
        let root = Self::build(&values, leaves, 19);
        ParameterList { values, root, id: NEXT_TREE_ID.fetch_add(1, Ordering::Relaxed) }
    }

    fn space<'a>(values: &'a [(ParameterPoint, T)], node: &'a Node) -> &'a [Parameter; 7] {
        match node {
            Node::Leaf(i) => &values[*i].0.space,
            Node::SubTree { space, .. } => space,
        }
    }

    fn parameter_space(values: &[(ParameterPoint, T)], children: &[Node]) -> [Parameter; 7] {
        let mut bounds: [Option<Parameter>; 7] = [None; 7];
        for child in children {
            let space = Self::space(values, child);
            for d in 0..7 {
                bounds[d] = Some(space[d].span(bounds[d]));
            }
        }
        bounds.map(Option::unwrap)
    }

    fn sort(values: &[(ParameterPoint, T)], nodes: &mut [Node], dimension: usize, absolute: bool) {
        let key = |node: &Node| {
            let space = Self::space(values, node);
            let mut key = [0i64; 7];
            for (k, item) in key.iter_mut().enumerate() {
                let c = center(space[(dimension + k) % 7]);
                *item = if absolute { c.abs() } else { c };
            }
            key
        };
        nodes.sort_by_key(key);
    }

    fn build(values: &[(ParameterPoint, T)], mut children: Vec<Node>, per_node: usize) -> Node {
        assert!(!children.is_empty(), "Need at least one child to build a node");
        if children.len() == 1 {
            return children.pop().unwrap();
        }
        if children.len() <= per_node {
            children.sort_by_key(|node| Self::space(values, node).iter().map(|p| center(*p).abs()).sum::<i64>());
            let space = Self::parameter_space(values, &children);
            return Node::SubTree { space, children };
        }
        let mut min_cost = i64::MAX;
        let mut min_dimension = 0;
        let mut min_order: Vec<usize> = Vec::new();
        let mut min_bucket_sizes: Vec<usize> = Vec::new();
        let mut nodes: Vec<(usize, Node)> = children.into_iter().enumerate().collect();
        for d in 0..7 {
            let key = |node: &Node| {
                let space = Self::space(values, node);
                let mut key = [0i64; 7];
                for (k, item) in key.iter_mut().enumerate() {
                    *item = center(space[(d + k) % 7]);
                }
                key
            };
            nodes.sort_by_key(|(_, n)| key(n));
            let sizes = Self::bucket_sizes(nodes.len(), per_node);
            let mut total = 0;
            let mut start = 0;
            for &size in &sizes {
                let bucket: Vec<&Node> = nodes[start..start + size].iter().map(|(_, n)| n).collect();
                let mut bounds: [Option<Parameter>; 7] = [None; 7];
                for child in bucket {
                    let space = Self::space(values, child);
                    for dim in 0..7 {
                        bounds[dim] = Some(space[dim].span(bounds[dim]));
                    }
                }
                total += cost(&bounds.map(Option::unwrap));
                start += size;
            }
            if min_cost > total {
                min_cost = total;
                min_dimension = d;
                min_order = nodes.iter().map(|(i, _)| *i).collect();
                min_bucket_sizes = sizes;
            }
        }
        let mut slots: Vec<Option<Node>> = Vec::new();
        slots.resize_with(nodes.len(), || None);
        for (i, node) in nodes {
            slots[i] = Some(node);
        }
        let mut buckets = Vec::new();
        let mut order = min_order.into_iter();
        for size in min_bucket_sizes {
            let bucket: Vec<Node> = (&mut order).take(size).map(|i| slots[i].take().unwrap()).collect();
            let space = Self::parameter_space(values, &bucket);
            buckets.push(Node::SubTree { space, children: bucket });
        }
        Self::sort(values, &mut buckets, min_dimension, true);
        let children = buckets
            .into_iter()
            .map(|bucket| match bucket {
                Node::SubTree { children, .. } => Self::build(values, children, per_node),
                Node::Leaf(_) => unreachable!(),
            })
            .collect::<Vec<_>>();
        let space = Self::parameter_space(values, &children);
        Node::SubTree { space, children }
    }

    fn bucket_sizes(count: usize, per_node: usize) -> Vec<usize> {
        let expected = (per_node as f64).powf((((count as f64) - 0.01).ln() / (per_node as f64).ln()).floor()) as usize;
        let mut sizes = Vec::new();
        let mut remaining = count;
        while remaining > 0 {
            let size = remaining.min(expected);
            sizes.push(size);
            remaining -= size;
        }
        sizes
    }

    fn distance(values: &[(ParameterPoint, T)], node: &Node, target: &TargetPoint) -> i64 {
        let space = Self::space(values, node);
        let mut distance = 0i64;
        for i in 0..7 {
            let d = space[i].distance(target[i]);
            distance = distance.wrapping_add(d.wrapping_mul(d));
        }
        distance
    }

    fn search_node(&self, node: &Node, target: &TargetPoint, candidate: Option<usize>) -> Option<usize> {
        let children = match node {
            Node::Leaf(i) => return Some(*i),
            Node::SubTree { children, .. } => children,
        };
        let leaf_distance = |leaf: usize| {
            let space = &self.values[leaf].0.space;
            let mut distance = 0i64;
            for i in 0..7 {
                let d = space[i].distance(target[i]);
                distance = distance.wrapping_add(d.wrapping_mul(d));
            }
            distance
        };
        let mut min_distance = candidate.map_or(i64::MAX, leaf_distance);
        let mut closest = candidate;
        for child in children {
            let child_distance = Self::distance(&self.values, child, target);
            if min_distance > child_distance {
                let leaf = self.search_node(child, target, closest).unwrap();
                let distance = if matches!(child, Node::Leaf(i) if *i == leaf) { child_distance } else { leaf_distance(leaf) };
                if min_distance > distance {
                    min_distance = distance;
                    closest = Some(leaf);
                }
            }
        }
        closest
    }

    pub fn find_value(&self, target: &TargetPoint) -> &T {
        let last = LAST_RESULT.with(|l| l.borrow().get(&self.id).copied());
        let leaf = self.search_node(&self.root, target, last).expect("a leaf");
        LAST_RESULT.with(|l| l.borrow_mut().insert(self.id, leaf));
        &self.values[leaf].1
    }
}
