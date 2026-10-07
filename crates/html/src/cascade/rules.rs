//! CSS rule extraction, specificity and declaration priority.

use super::csstree::{self, Important, Node};
use super::shorthand::expand_static_declaration;
use impeccino_core::js;
use indexmap::IndexMap;
use regex::Regex;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::LazyLock as Lazy;

/// The cascade metadata carried by a specified declaration
/// (`{ important, specificity, order, inline }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclMeta {
    pub important: bool,
    pub specificity: [u32; 3],
    pub order: i64,
    pub inline: bool,
}

/// A winning declaration in the specified store: `{ ...meta, prop, value }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecifiedDecl {
    pub meta: DeclMeta,
    pub prop: String,
    pub value: String,
}

/// True when `b` should replace the existing `a` (or there is no `a`).
pub fn compare_static_priority(a: Option<&DeclMeta>, b: &DeclMeta) -> bool {
    let Some(a) = a else {
        return true;
    };
    if b.important != a.important {
        return b.important;
    }
    if b.inline != a.inline {
        return b.inline;
    }
    for i in 0..3 {
        if b.specificity[i] != a.specificity[i] {
            return b.specificity[i] > a.specificity[i];
        }
    }
    b.order >= a.order
}

static WHERE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r":where\([^)]*\)").expect("WHERE_RE"));
static ID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"#[0-9A-Za-z_-]+").expect("ID_RE"));
// JS `:(?!:)[\w-]+`: the lookahead is implied because `[\w-]` excludes `:`.
static CLASS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\.[0-9A-Za-z_-]+|\[[^\]]+\]|:[0-9A-Za-z_-]+(?:\([^)]*\))?").expect("CLASS_RE")
});
static CLASS_STRIP_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\.[0-9A-Za-z_-]+|\[[^\]]+\]|:{1,2}[0-9A-Za-z_-]+(?:\([^)]*\))?")
        .expect("CLASS_STRIP_RE")
});
static PUNCT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[*>+~(),]").expect("PUNCT_RE"));
static TYPE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?-u:\b)[a-zA-Z][0-9A-Za-z_-]*(?-u:\b)").expect("TYPE_RE"));

/// `staticSpecificity`: -> [ids, classes, types]
pub fn static_specificity(selector: &str) -> [u32; 3] {
    let no_where = WHERE_RE.replace_all(selector, "");
    let ids = ID_RE.find_iter(&no_where).count() as u32;
    let classes = CLASS_RE.find_iter(&no_where).count() as u32;
    let stripped = ID_RE.replace_all(&no_where, " ");
    let stripped = CLASS_STRIP_RE.replace_all(&stripped, " ");
    let stripped = PUNCT_RE.replace_all(&stripped, " ");
    let types = TYPE_RE.find_iter(&stripped).count() as u32;
    [ids, classes, types]
}

/// The JS `specified` map: node -> (expanded prop -> winning declaration).
/// Generic over the node key so the DOM engine can key it by node id.
#[derive(Debug, Clone)]
pub struct SpecifiedStore<K: Hash + Eq> {
    map: HashMap<K, IndexMap<String, SpecifiedDecl>>,
}

impl<K: Hash + Eq> Default for SpecifiedStore<K> {
    fn default() -> Self {
        SpecifiedStore {
            map: HashMap::new(),
        }
    }
}

impl<K: Hash + Eq> SpecifiedStore<K> {
    pub fn new() -> Self {
        Self::default()
    }
    /// JS `specified.get(node)`: the per-property winners in insertion order.
    pub fn get(&self, node: &K) -> Option<&IndexMap<String, SpecifiedDecl>> {
        self.map.get(node)
    }
    pub fn get_mut(&mut self, node: &K) -> Option<&mut IndexMap<String, SpecifiedDecl>> {
        self.map.get_mut(node)
    }
    pub fn contains(&self, node: &K) -> bool {
        self.map.contains_key(node)
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&K, &IndexMap<String, SpecifiedDecl>)> {
        self.map.iter()
    }
}

pub fn apply_static_declaration<K: Hash + Eq>(
    specified: &mut SpecifiedStore<K>,
    node: K,
    prop: &str,
    value: &str,
    meta: &DeclMeta,
) {
    let map = specified.map.entry(node).or_default();
    for (expanded_prop, expanded_value) in expand_static_declaration(prop, value) {
        let existing = map.get(&expanded_prop).map(|d| &d.meta);
        if compare_static_priority(existing, meta) {
            let next = SpecifiedDecl {
                meta: meta.clone(),
                prop: expanded_prop.clone(),
                value: expanded_value,
            };
            map.insert(expanded_prop, next);
        }
    }
}

/// One declaration from a `style=""` attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleAttrDecl {
    pub prop: String,
    pub value: String,
    pub important: bool,
    pub order: i64,
}

static IMPORTANT_TAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r"(?i)!important{ws}*$", ws = js::WS)).expect("IMPORTANT_TAIL_RE")
});
static IMPORTANT_STRIP_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r"(?i){ws}*!important{ws}*$", ws = js::WS)).expect("IMPORTANT_STRIP_RE")
});

pub fn parse_static_style_attribute(style_text: &str, order_base: i64) -> Vec<StyleAttrDecl> {
    let mut decls: Vec<StyleAttrDecl> = Vec::new();
    for part in style_text.split(';') {
        let Some(idx) = part.find(':') else {
            continue;
        };
        if idx == 0 {
            continue;
        }
        let prop = js::trim(&part[..idx]).to_string();
        let mut value = js::trim(&part[idx + 1..]).to_string();
        let important = IMPORTANT_TAIL_RE.is_match(&value);
        value = js::trim(&IMPORTANT_STRIP_RE.replace(&value, "")).to_string();
        let order = order_base + decls.len() as i64;
        decls.push(StyleAttrDecl {
            prop,
            value,
            important,
            order,
        });
    }
    decls
}

/// A declaration inside a collected rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDecl {
    pub prop: String,
    pub value: String,
    pub important: bool,
}

/// One entry of `collectStaticCssRules`' output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssRule {
    pub selector: String,
    pub declarations: Vec<RuleDecl>,
    pub specificity: [u32; 3],
    pub order: i64,
    pub is_hover: bool,
    /// The state-stripped selector for :hover rules (`None` when the rule
    /// is not a hover rule or the stripped selector is unusable).
    pub match_selector: Option<String>,
}

static HOVER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i):hover(?-u:\b)").expect("HOVER_RE"));
static TRAILING_COMBINATOR_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r"[>+~]{ws}*$", ws = js::WS)).expect("TRAILING_COMBINATOR_RE")
});

fn is_combinator_or_ws(c: char) -> bool {
    js::is_js_whitespace(c) || c == '>' || c == '+' || c == '~'
}

/// JS `matchSelector.replace(/(^|[\s>+~])(?=$|[\s>+~])/g, '$1*')`, done by
/// hand because the regex crate has no lookahead. Walks the string the way
/// a global replace does: at each position try `^` (only at 0), then one
/// combinator/whitespace char; the lookahead requires end-of-string or a
/// combinator/whitespace char right after group 1. An empty match inserts
/// `*` and advances one char; a one-char match appends `*` after it.
fn star_empty_compounds(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut pos = 0usize;
    let look = |at: usize| -> bool { at >= n || is_combinator_or_ws(chars[at]) };
    while pos <= n {
        if pos == 0 && look(0) {
            // `^` alternative: empty match, replacement is `*`.
            out.push('*');
            if n == 0 {
                break;
            }
            out.push(chars[0]);
            pos = 1;
            continue;
        }
        if pos < n && is_combinator_or_ws(chars[pos]) && look(pos + 1) {
            out.push(chars[pos]);
            out.push('*');
            pos += 1;
            continue;
        }
        if pos < n {
            out.push(chars[pos]);
        }
        pos += 1;
    }
    out
}

pub fn collect_static_css_rules(css_text: &str) -> Vec<CssRule> {
    let mut rules: Vec<CssRule> = Vec::new();
    let ast = match csstree::parse_stylesheet(css_text) {
        Ok(ast) => ast,
        Err(_) => return rules,
    };
    let mut order: i64 = 0;
    let Node::StyleSheet { children } = &ast else {
        return rules;
    };
    walk_list(children, &[], &mut rules, &mut order);
    rules
}

fn walk_list(list: &[Node], at_rule_stack: &[String], rules: &mut Vec<CssRule>, order: &mut i64) {
    for node in list {
        match node {
            Node::Rule { prelude, block } => {
                if at_rule_stack
                    .iter()
                    .any(|name| js::to_lower_case(name).ends_with("keyframes"))
                {
                    continue;
                }
                let selector_text = csstree::generate(prelude);
                let selector_text = js::trim(&selector_text);
                let mut declarations: Vec<RuleDecl> = Vec::new();
                if let Node::Block { children } = &**block {
                    for child in children {
                        if let Node::Declaration {
                            important,
                            property,
                            value,
                        } = child
                        {
                            let generated = csstree::generate(value);
                            declarations.push(RuleDecl {
                                prop: property.clone(),
                                value: js::trim(&generated).to_string(),
                                important: matches!(
                                    important,
                                    Important::Yes | Important::Other(_)
                                ),
                            });
                        }
                    }
                }
                for selector in super::values::split_css_list(selector_text) {
                    if selector.is_empty() {
                        continue;
                    }
                    // :hover rules can't be matched statically as-is (no
                    // interaction state), but they carry real cascade weight
                    // while hovered. Tag them and record a state-stripped
                    // selector so the hover pass can find their targets;
                    // specificity stays computed from the ORIGINAL selector
                    // (per CSS, :hover counts as a class).
                    let is_hover = HOVER_RE.is_match(&selector);
                    let mut match_selector: Option<String> = None;
                    if is_hover {
                        let stripped = HOVER_RE.replace_all(&selector, "");
                        let stripped = js::trim(&stripped).to_string();
                        if stripped.is_empty() || TRAILING_COMBINATOR_RE.is_match(&stripped) {
                            match_selector = None;
                        } else {
                            match_selector = Some(star_empty_compounds(&stripped));
                        }
                    }
                    let specificity = static_specificity(&selector);
                    rules.push(CssRule {
                        selector,
                        declarations: declarations.clone(),
                        specificity,
                        order: *order,
                        is_hover,
                        match_selector,
                    });
                    *order += 1;
                }
            }
            Node::Atrule {
                name,
                block: Some(block),
                ..
            } => {
                let lower = js::to_lower_case(name);
                if lower == "media" || lower == "supports" || lower == "layer" {
                    if let Node::Block { children } = &**block {
                        let mut stack: Vec<String> = at_rule_stack.to_vec();
                        stack.push(lower);
                        walk_list(children, &stack, rules, order);
                    }
                }
            }
            _ => {}
        }
    }
}
