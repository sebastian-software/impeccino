//! `impeccino concept-seed`: a repeatable suggestion over the model's own
//! ordered candidates. Everything is local: the model writes the
//! ordered list, a hash of the seed key picks the index, and nothing leaves
//! the machine (docs/adr/0019-concept-seed-is-local.md).

use crate::context::load_context_without_visual_scan;
use crate::seed_text as t;
use crate::target_args::TargetOptions;
use impeccino_common::Io;
use sha2::{Digest, Sha256};

pub const SEED_MODES: [&str; 4] = ["persuade", "operate", "read", "experience"];

/// Flags that drove the retired choice ping. Older skill text may still pass
/// them after a choice, so they get a clear answer instead of a silent no-op.
const REMOVED_FLAGS: [&str; 2] = ["--chosen", "--kind"];

fn fill(tpl: &str, pairs: &[(&str, &str)]) -> String {
    let mut s = tpl.to_string();
    for (k, v) in pairs {
        s = s.replace(&format!("@@{}@@", k), v);
    }
    s
}

pub struct SeedArgs {
    pub scope: Option<String>,
    pub key: String,
    pub reroll: f64,
    pub register: Option<Option<String>>, // None = not given; Some(None) = flag without value
    pub mode: Option<Option<String>>,
    pub candidate_count: f64,
}

fn unit(scope: &str, salt: &str, key: &str) -> f64 {
    let d = Sha256::digest(format!("{}:{}:{}", scope, salt, key).as_bytes());
    unit_value(u32::from_be_bytes([d[0], d[1], d[2], d[3]]))
}

fn unit_value(value: u32) -> f64 {
    value as f64 / 4294967296.0
}

/// The roll itself: the index to build, and for a surface round the three
/// dealt indices (the build index leads). Pure, so the same key always deals
/// the same hand.
fn deal(scope: &str, key: &str, reroll: usize, candidate_count: usize) -> (usize, Vec<usize>) {
    let index_salt = if reroll == 0 {
        "index".to_string()
    } else {
        format!("index:reroll-{}", reroll)
    };
    let build_index =
        3 + (unit(scope, &index_salt, key) * (candidate_count as f64 - 2.0)).floor() as usize;
    let mut dealt: Vec<usize> = vec![build_index];
    let want = 3.min(candidate_count);
    let mut draw = 0usize;
    while scope == "surface" && dealt.len() < want {
        let idx = 1
            + (unit(scope, &format!("{}:deal-{}", index_salt, draw), key) * candidate_count as f64)
                .floor() as usize;
        if !dealt.contains(&idx) {
            dealt.push(idx);
        }
        if draw > 64 {
            let mut f = 1;
            while dealt.len() < want {
                if !dealt.contains(&f) {
                    dealt.push(f);
                }
                f += 1;
            }
        }
        draw += 1;
    }
    (build_index, dealt)
}

fn render_concept_seed(a: &SeedArgs) -> Result<String, String> {
    let scope = match a.scope.as_deref() {
        Some("surface") => "surface",
        Some("direction") => "direction",
        _ => return Err("concept-seed: --scope must be direction or surface".into()),
    };
    if !(a.reroll.is_finite() && a.reroll.fract() == 0.0) || a.reroll < 0.0 {
        return Err("concept-seed: --reroll must be a non-negative integer".into());
    }
    let reroll = a.reroll as usize;
    let register: Option<&str> = match &a.register {
        None => None,
        Some(Some(r)) if r == "safer" || r == "bolder" => Some(r.as_str()),
        Some(_) => return Err("concept-seed: --register must be safer or bolder".into()),
    };
    if register.is_some() && reroll < 1 {
        return Err(
            "concept-seed: --register steers a re-roll round; pass --reroll <n> with it".into(),
        );
    }
    if register.is_some() && scope != "direction" {
        return Err("concept-seed: --register applies to direction rounds only".into());
    }
    let mode: Option<&str> = match &a.mode {
        None => None,
        Some(Some(m)) if SEED_MODES.contains(&m.as_str()) => Some(m.as_str()),
        Some(_) => {
            return Err(
                "concept-seed: --mode must be persuade, operate, read, or experience".into(),
            )
        }
    };
    if !(a.candidate_count.is_finite() && a.candidate_count.fract() == 0.0)
        || a.candidate_count < 5.0
        || a.candidate_count > 7.0
    {
        return Err("concept-seed: --candidate-count must be an integer from 5 to 7".into());
    }
    let candidate_count = a.candidate_count as usize;
    let key = a.key.as_str();
    let (build_index, dealt) = deal(scope, key, reroll, candidate_count);
    let dealt_str = dealt
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let mode_flag = mode.map(|m| format!(" --mode {}", m)).unwrap_or_default();
    let reroll_flag = if reroll > 0 {
        format!(" --reroll {}", reroll)
    } else {
        String::new()
    };
    let register_flag = register
        .map(|r| format!(" --register {}", r))
        .unwrap_or_default();
    let mode_or_unscoped = mode.unwrap_or("unscoped").to_string();
    let scope_upper = scope.to_uppercase();
    let build_index_s = build_index.to_string();
    let count_s = candidate_count.to_string();
    let reroll_s = reroll.to_string();
    let pairs: Vec<(&str, &str)> = vec![
        ("SCOPE_UPPER", &scope_upper),
        ("SCOPE", scope),
        ("KEY", key),
        ("MODE_OR_UNSCOPED", &mode_or_unscoped),
        ("MODE_FLAG", &mode_flag),
        ("REROLL_FLAG", &reroll_flag),
        ("REGISTER_FLAG", &register_flag),
        ("CANDIDATECOUNT", &count_s),
        ("BUILDINDEX", &build_index_s),
        ("DEALT_INDICES", &dealt_str),
        ("REROLL", &reroll_s),
    ];
    let authority = if scope == "direction" {
        t::AUTHORITY_DIRECTION
    } else {
        t::AUTHORITY_SURFACE
    };

    let mut out = fill(t::HEADER, &pairs);
    out.push('\n');
    if register == Some("safer") {
        // The safer register suspends the assignment: the user picks from the
        // conventional end, so no index is printed.
        out.push_str(t::SAFER_BLOCK);
        out.push_str(authority);
        out.push('\n');
        out.push_str(t::PINNED);
        out.push_str(&fill(t::RESTATED_SAFER, &pairs));
        return Ok(out);
    }
    if reroll > 0 {
        out.push_str(&fill(t::REROLL_BLOCK, &pairs));
    }
    if register == Some("bolder") {
        out.push_str(t::BOLDER_BLOCK);
    }
    let (assigned_or_dealt, promoted, restated) = if scope == "direction" {
        (
            format!("ASSIGNED INDEX: {}", build_index),
            fill(t::PROMOTED_DIRECTION, &pairs),
            fill(t::RESTATED_DIRECTION, &pairs),
        )
    } else {
        (
            format!("DEALT INDICES: {} (index {} leads)", dealt_str, build_index),
            fill(t::PROMOTED_SURFACE, &pairs),
            fill(t::RESTATED_SURFACE, &pairs),
        )
    };
    out.push_str(&fill(
        t::ASSIGNED_BLOCK,
        &[
            ("ASSIGNED_OR_DEALT", &assigned_or_dealt),
            ("PROMOTEDINSTRUCTION", &promoted),
        ],
    ));
    if scope == "direction" {
        out.push_str(t::CHANNEL_DIRECTION);
    }
    out.push_str(authority);
    out.push('\n');
    out.push_str(t::PINNED);
    out.push_str(&restated);
    Ok(out)
}

fn random_hex8() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut x = (t as u64) ^ ((std::process::id() as u64) << 32) ^ 0x9E3779B97F4A7C15;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    format!("{:08x}", (x & 0xffffffff) as u32)
}

pub fn run(args: &[String], io: &mut Io) -> i32 {
    let cwd = io.cwd.to_string_lossy().into_owned();
    let env = io.env.clone();
    let idx = |name: &str| args.iter().position(|a| a == name);
    // args[idx+1] may be undefined -> None
    let val =
        |name: &str| -> Option<Option<String>> { idx(name).map(|i| args.get(i + 1).cloned()) };
    if val("--from").is_some_and(|value| value.is_none_or(|key| key.starts_with("--"))) {
        io.err("--from requires a seed key.\n");
        return 1;
    }
    if let Some(flag) = REMOVED_FLAGS.iter().find(|f| idx(f).is_some()) {
        io.err(&fill(t::REMOVED_FLAG, &[("FLAG", flag)]));
        return 1;
    }
    let ctx = load_context_without_visual_scan(&cwd, &TargetOptions::default(), &env);
    if !ctx.has_product {
        io.out(t::NO_PRODUCT);
        return 1;
    }
    let num = |v: Option<Option<String>>| -> Option<f64> {
        v.map(|x| x.map(|s| crate::util::js_number(&s)).unwrap_or(f64::NAN))
    };
    let seed = SeedArgs {
        scope: match val("--scope") {
            None => Some("surface".to_string()),
            Some(v) => v, // flag without a value -> None -> invalid scope
        },
        key: match val("--from") {
            Some(Some(k)) => k,
            Some(None) => unreachable!("missing seed value was rejected"),
            None => env
                .get("IMPECCINO_CONCEPT_SEED")
                .filter(|v| !v.is_empty())
                .cloned()
                .unwrap_or_else(random_hex8),
        },
        reroll: num(val("--reroll")).unwrap_or(0.0),
        register: val("--register"),
        mode: val("--mode"),
        candidate_count: num(val("--candidate-count")).unwrap_or(7.0),
    };
    match render_concept_seed(&seed) {
        Ok(text) => {
            io.out(&text);
            0
        }
        Err(msg) => {
            io.err(&format!("{}\n", msg));
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(scope: &str, key: &str, reroll: f64, register: Option<&str>) -> SeedArgs {
        SeedArgs {
            scope: Some(scope.to_string()),
            key: key.to_string(),
            reroll,
            register: register.map(|r| Some(r.to_string())),
            mode: Some(Some("persuade".to_string())),
            candidate_count: 7.0,
        }
    }

    #[test]
    fn maximum_rng_value_stays_below_one() {
        assert!(unit_value(u32::MAX) < 1.0);
    }

    #[test]
    fn a_seed_flag_without_a_value_is_rejected_before_loading_context() {
        let (mut io, captured) =
            Io::captured("", std::env::temp_dir(), std::collections::HashMap::new());
        assert_eq!(run(&["--from".to_string()], &mut io), 1);
        assert!(String::from_utf8_lossy(&captured.stderr.borrow())
            .contains("--from requires a seed key"));
    }

    #[test]
    fn same_key_deals_the_same_hand() {
        assert_eq!(deal("direction", "k", 0, 7), deal("direction", "k", 0, 7));
        assert_eq!(deal("surface", "k", 2, 5), deal("surface", "k", 2, 5));
    }

    #[test]
    fn the_assignment_never_lands_on_the_top_two() {
        for i in 0..200 {
            for count in 5..=7 {
                let (build, _) = deal("direction", &format!("key-{i}"), 0, count);
                assert!((3..=count).contains(&build), "{build} out of 3..={count}");
            }
        }
    }

    #[test]
    fn a_surface_round_deals_three_distinct_indices_led_by_the_build_index() {
        for i in 0..200 {
            let (build, dealt) = deal("surface", &format!("key-{i}"), 1, 5);
            assert_eq!(dealt.len(), 3);
            assert_eq!(dealt[0], build);
            let mut sorted = dealt.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), 3);
            assert!(dealt.iter().all(|d| (1..=5).contains(d)));
        }
    }

    #[test]
    fn output_names_no_service_catalog_or_ping() {
        for a in [
            args("direction", "k", 0.0, None),
            args("direction", "k", 1.0, Some("bolder")),
            args("direction", "k", 1.0, Some("safer")),
            args("surface", "k", 0.0, None),
        ] {
            let out = render_concept_seed(&a).unwrap();
            for banned in [
                "challenger",
                "catalog",
                "QUALITY BAR",
                "impeccable.style",
                "network",
                "TELEMETRY",
                "degraded",
                "--chosen",
                "--kind",
            ] {
                assert!(!out.contains(banned), "{banned} in:\n{out}");
            }
        }
    }

    #[test]
    fn safer_suspends_the_assignment() {
        let out = render_concept_seed(&args("direction", "k", 1.0, Some("safer"))).unwrap();
        assert!(out.contains("SAFER REGISTER"));
        assert!(!out.contains("ASSIGNED INDEX"));
    }

    #[test]
    fn bolder_keeps_the_assignment() {
        let out = render_concept_seed(&args("direction", "k", 1.0, Some("bolder"))).unwrap();
        assert!(out.contains("BOLDER REGISTER"));
        assert!(out.contains("RE-ROLL ROUND 1"));
        assert!(out.contains("ASSIGNED INDEX:"));
    }
}
