use crate::advancements::advancement::Advancement;
use crate::advancements::advancement_node::AdvancementNode;
use crate::advancements::advancement_tree::AdvancementTree;

const VISIBILITY_DEPTH: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisibilityRule {
    Show,
    Hide,
    NoChange,
}

fn evaluate_visibility_rule(advancement: &Advancement, is_done: bool) -> VisibilityRule {
    match &advancement.display {
        None => VisibilityRule::Hide,
        Some(_) if is_done => VisibilityRule::Show,
        Some(display) => {
            if display.hidden {
                VisibilityRule::Hide
            } else {
                VisibilityRule::NoChange
            }
        }
    }
}

fn evaluate_visiblity_for_unfinished_node(ascendants: &[VisibilityRule]) -> bool {
    for i in 0..=VISIBILITY_DEPTH {
        match ascendants[ascendants.len() - 1 - i] {
            VisibilityRule::Show => return true,
            VisibilityRule::Hide => return false,
            VisibilityRule::NoChange => {}
        }
    }
    false
}

fn evaluate_visibility_recursive<'a>(
    tree: &'a AdvancementTree,
    node: &'a AdvancementNode,
    ascendants: &mut Vec<VisibilityRule>,
    is_done_test: &impl Fn(&'a AdvancementNode) -> bool,
    output: &mut impl FnMut(&'a AdvancementNode, bool),
) -> bool {
    let is_self_done = is_done_test(node);
    let descendant_visibility = evaluate_visibility_rule(node.advancement(), is_self_done);
    let mut is_self_or_descendant_done = is_self_done;
    ascendants.push(descendant_visibility);
    for child in tree.children(node) {
        is_self_or_descendant_done |= evaluate_visibility_recursive(tree, child, ascendants, is_done_test, output);
    }
    let visibility = is_self_or_descendant_done || evaluate_visiblity_for_unfinished_node(ascendants);
    ascendants.pop();
    output(node, visibility);
    is_self_or_descendant_done
}

pub fn evaluate_visibility<'a>(tree: &'a AdvancementTree, node: &'a AdvancementNode, is_done: impl Fn(&'a AdvancementNode) -> bool, mut output: impl FnMut(&'a AdvancementNode, bool)) {
    let root = tree.root(node);
    let mut visibility_stack = Vec::new();
    for _ in 0..=VISIBILITY_DEPTH {
        visibility_stack.push(VisibilityRule::NoChange);
    }
    evaluate_visibility_recursive(tree, root, &mut visibility_stack, &is_done, &mut output);
}
