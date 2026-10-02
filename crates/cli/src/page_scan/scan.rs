//! The rendered-page scan, run against a page agent-browser holds.
//!
//! Every page operation (capture a snapshot, answer hit tests, scroll, load
//! an image, read a pixel, take a screenshot) is a request sent through
//! [`PageIo`]. The decisions all stay here and in `impeccino_core`: this is
//! the former URL engine's scan (`crates/browser`, docs/adr/0011) with the
//! Chrome DevTools connection replaced by agent-browser (docs/adr/0016).

use impeccino_core::browser::driver::{collect_browser_findings, serialize_findings};
use impeccino_core::browser::page_checks::measure_hidden_text_dom;
use impeccino_core::browser::snapshot::{Facts, SnapshotDom};
use impeccino_core::browser::visual::{self, CssPlan, Prepared, StackNode};
use impeccino_core::browser::{BrowserConfig, Dom, ElId};
use impeccino_core::checks::measures::{check_content_hidden_at_rest, ContentHiddenInput};
use impeccino_core::color::Rgba;
use serde_json::{json, Value};

/// How many low-contrast candidates the visual pass analyzes, as the URL
/// engine did.
const MAX_VISUAL_CANDIDATES: f64 = 12.0;
/// How many script errors become findings, as the URL engine reported.
const MAX_SCRIPT_ERRORS: usize = 3;
const MAX_HIT_TEST_ROUNDS: usize = 12;

/// One request to the page and its JSON answer.
pub trait PageIo {
    fn call(&mut self, op: Value) -> Result<Value, String>;
}

/// A finding before it is resolved against the registry.
pub struct RawResult {
    pub id: String,
    pub snippet: String,
    pub ignore_value: String,
    pub severity: String,
}

impl RawResult {
    fn plain(id: impl Into<String>, snippet: impl Into<String>) -> Self {
        RawResult { id: id.into(), snippet: snippet.into(), ignore_value: String::new(), severity: String::new() }
    }
}

/// The full scan: the browser rules, content hidden at rest after a reveal
/// sweep, script errors, and the visual-contrast pass.
pub fn scan(io: &mut dyn PageIo, config: &BrowserConfig) -> Result<Vec<RawResult>, String> {
    let dom = capture(io)?;
    let collected = resolve_needs(&dom, io, |d| collect_browser_findings(d, config))?;
    let groups = serialize_findings(&dom, &collected.groups).as_array().cloned().unwrap_or_default();
    let mut results = Vec::new();
    for group in &groups {
        for f in group.get("findings").and_then(Value::as_array).into_iter().flatten() {
            results.push(RawResult {
                id: str_of(f.get("type")),
                snippet: str_of(f.get("detail")),
                ignore_value: str_of(f.get("ignoreValue")),
                severity: str_of(f.get("severity")),
            });
        }
    }

    // Reveal lazy and on-scroll content, then measure what stays hidden from
    // the scroll-0 capture the visual pass shares.
    io.call(json!({ "op": "reveal" }))?;
    let base = capture(io)?;
    let measured = resolve_needs(&base, io, |d| measure_hidden_text_dom(d))?;
    let input = ContentHiddenInput {
        total_chars: measured.total_chars,
        hidden_chars: measured.hidden_chars,
        hidden_samples: measured.hidden_samples,
    };
    results.extend(check_content_hidden_at_rest(&input).into_iter().map(|f| RawResult::plain(f.id, f.snippet)));

    let errors = io.call(json!({ "op": "errors" }))?;
    for message in errors.as_array().into_iter().flatten().filter_map(Value::as_str).take(MAX_SCRIPT_ERRORS) {
        results.push(RawResult::plain("script-error", message));
    }

    let analyses = analyze_visual_contrast(io, &base)?;
    results.extend(visual_findings(&analyses, &groups));
    results.extend(screenshot_fallback(io, &analyses, &groups)?);
    Ok(results)
}

fn str_of(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or("").to_string()
}

// ─── snapshot ──────────────────────────────────────────────────────────────

fn capture(io: &mut dyn PageIo) -> Result<SnapshotDom, String> {
    let out = io.call(json!({ "op": "capture" }))?;
    if let Some(err) = out.get("error").and_then(Value::as_str) {
        return Err(format!("page capture failed: {err}"));
    }
    let json = out.get("json").and_then(Value::as_str).ok_or("page capture returned no snapshot")?;
    SnapshotDom::from_json(json).map_err(|e| format!("unreadable page snapshot: {e}"))
}

/// Re-measure only the scroll-dependent geometry after a scroll and patch it
/// onto a clone of `base` (ids stay document order, so they still match).
fn recapture_geometry(io: &mut dyn PageIo, base: &SnapshotDom) -> Result<SnapshotDom, String> {
    let out = io.call(json!({ "op": "geometry" }))?;
    let mut snap = base.snap.clone();
    if out.is_object() {
        snap.scroll_x = out.get("scrollX").and_then(Value::as_f64).unwrap_or(snap.scroll_x);
        snap.scroll_y = out.get("scrollY").and_then(Value::as_f64).unwrap_or(snap.scroll_y);
        let rects = out.get("rects").and_then(Value::as_array);
        let dtrs = out.get("dtrs").and_then(Value::as_array);
        let rect4 = |v: Option<&Value>| -> Option<[f64; 4]> {
            let a = v?.as_array()?;
            if a.len() < 4 {
                return None;
            }
            Some([a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?, a[3].as_f64()?])
        };
        for (i, node) in snap.els.iter_mut().enumerate() {
            if let Some(cell) = rects.and_then(|r| r.get(i)) {
                node.rect = rect4(Some(cell));
            }
            if let Some(cell) = dtrs.and_then(|d| d.get(i)) {
                node.direct_text_rect = rect4(Some(cell));
            }
        }
    }
    Ok(SnapshotDom::new(snap))
}

/// Run `f`, and while it asked hit tests the snapshot cannot answer, ask the
/// page and run it again (deterministic runs converge in a round or two).
fn resolve_needs<T>(dom: &SnapshotDom, io: &mut dyn PageIo, f: impl Fn(&SnapshotDom) -> T) -> Result<T, String> {
    let mut out = f(dom);
    let mut rounds = 0;
    while dom.has_needs() && rounds < MAX_HIT_TEST_ROUNDS {
        let needs = dom.take_needs();
        let facts = io.call(json!({ "op": "answer", "hitTests": needs.hit_tests }))?;
        dom.add_facts(&serde_json::from_value::<Facts>(facts).unwrap_or_default());
        out = f(dom);
        rounds += 1;
    }
    let _ = dom.take_needs();
    Ok(out)
}

// ─── visual contrast ───────────────────────────────────────────────────────

struct LoadedImage {
    reference: Value,
    w: f64,
    h: f64,
}

fn load_image(io: &mut dyn PageIo, src: &str) -> Result<Option<LoadedImage>, String> {
    let out = io.call(json!({ "op": "loadImage", "src": src }))?;
    if out.is_null() || out.get("error").is_some() {
        return Ok(None);
    }
    Ok(Some(LoadedImage {
        reference: out.get("ref").cloned().unwrap_or(Value::Null),
        w: out.get("w").and_then(Value::as_f64).unwrap_or(0.0),
        h: out.get("h").and_then(Value::as_f64).unwrap_or(0.0),
    }))
}

fn live_scroll(io: &mut dyn PageIo) -> Result<(f64, f64), String> {
    let out = io.call(json!({ "op": "scroll" }))?;
    Ok((out.get("x").and_then(Value::as_f64).unwrap_or(0.0), out.get("y").and_then(Value::as_f64).unwrap_or(0.0)))
}

fn media(dom: &SnapshotDom, el: ElId) -> impeccino_core::browser::snapshot::MediaInfo {
    dom.snap.get(el).and_then(|n| n.media.clone()).unwrap_or_default()
}

/// `naturalWidth || videoWidth || width`.
fn intrinsic_img(dom: &SnapshotDom, el: ElId) -> (f64, f64) {
    let m = media(dom, el);
    (first_nonzero(&[m.nw, m.vw, m.w]), first_nonzero(&[m.nh, m.vh, m.h]))
}

/// `width || videoWidth`.
fn intrinsic_raster(dom: &SnapshotDom, el: ElId) -> (f64, f64) {
    let m = media(dom, el);
    (first_nonzero(&[m.w, m.vw]), first_nonzero(&[m.h, m.vh]))
}

fn img_src(dom: &SnapshotDom, el: ElId) -> String {
    let m = media(dom, el);
    if !m.cur.is_empty() { m.cur } else { m.src }
}

fn first_nonzero(vals: &[f64]) -> f64 {
    vals.iter().copied().find(|v| *v != 0.0 && !v.is_nan()).unwrap_or(0.0)
}

fn is_sampled(sample: &Value) -> bool {
    sample.get("status").and_then(Value::as_str) == Some("sampled")
}

fn sample_reason(sample: &Value) -> String {
    str_of(sample.get("reason"))
}

/// The raster plan and pixel address come from the core, the read from the page.
fn sample_drawable_pixel(io: &mut dyn PageIo, reference: &Value, intrinsic: (f64, f64), sx: f64, sy: f64) -> Result<Value, String> {
    let plan = visual::raster_plan(intrinsic.0, intrinsic.1);
    let (rpx, rpy) = visual::raster_pixel(&plan, sx, sy);
    let plan_json = serde_json::to_value(plan).unwrap_or(Value::Null);
    let read = io.call(json!({ "op": "readPixel", "ref": reference, "plan": plan_json, "x": rpx, "y": rpy }))?;
    if read.get("noContext").and_then(Value::as_bool) == Some(true) {
        return Ok(visual::raster_no_context_sample());
    }
    if let Some(err) = read.get("error") {
        return Ok(visual::raster_failure_sample(&visual::raster_error_reason(err.as_str().unwrap_or(""))));
    }
    let d = read.get("data").and_then(Value::as_array);
    let ch = |i: usize| d.and_then(|a| a.get(i)).and_then(Value::as_f64).unwrap_or(0.0);
    Ok(visual::pixel_sample(ch(0), ch(1), ch(2), ch(3)))
}

fn sample_image_element(io: &mut dyn PageIo, dom: &SnapshotDom, node: ElId, px: f64, py: f64) -> Result<Value, String> {
    let intrinsic = intrinsic_img(dom, node);
    let (painted, source) = match visual::img_source_point(dom, node, intrinsic.0, intrinsic.1, px, py) {
        Err(sample) => return Ok(sample),
        Ok(v) => v,
    };
    let sample = sample_drawable_pixel(io, &json!(node), intrinsic, source.0, source.1)?;
    let finished = visual::img_finish(sample.clone());
    if is_sampled(&finished) {
        return Ok(finished);
    }
    let src = img_src(dom, node);
    if !src.is_empty() {
        if let Some(loaded) = load_image(io, &src)? {
            if let Some(point) = visual::img_loaded_source_point(&painted, loaded.w, loaded.h, px, py) {
                let pixel = sample_drawable_pixel(io, &loaded.reference, (loaded.w, loaded.h), point.0, point.1)?;
                let loaded_sample = visual::img_finish(pixel);
                if is_sampled(&loaded_sample) {
                    return Ok(loaded_sample);
                }
            }
        }
    }
    Ok(sample)
}

fn sample_css_background(io: &mut dyn PageIo, dom: &SnapshotDom, node: ElId, px: f64, py: f64, text_color: &Rgba) -> Result<Value, String> {
    match visual::css_plan(dom, node, Some(text_color)) {
        CssPlan::Sample { sample } => Ok(sample),
        CssPlan::Url { url, size, position } => {
            let Some(img) = load_image(io, &url)? else {
                return Ok(visual::css_url_no_image());
            };
            match visual::css_url_source_point(dom, node, img.w, img.h, &size, &position, px, py) {
                Err(sample) => Ok(sample),
                Ok(source) => {
                    let pixel = sample_drawable_pixel(io, &img.reference, (img.w, img.h), source.0, source.1)?;
                    Ok(visual::css_url_finish(pixel))
                }
            }
        }
    }
}

fn sample_background(io: &mut dyn PageIo, dom: &SnapshotDom, el: ElId, px: f64, py: f64, depth: f64, text_color: &Rgba) -> Result<Value, String> {
    let walk = resolve_needs(dom, io, |d| visual::stack_nodes(d, el, px, py, depth))?;
    let nodes = match walk {
        Err(unresolved) => return Ok(unresolved),
        Ok(nodes) => nodes,
    };
    let mut unresolved: Vec<String> = Vec::new();
    for StackNode { el: node, kind } in nodes {
        match kind.as_str() {
            "img" => {
                let sample = sample_image_element(io, dom, node, px, py)?;
                if is_sampled(&sample) {
                    return Ok(sample);
                }
                unresolved.push(sample_reason(&sample));
            }
            "raster" => {
                let intrinsic = intrinsic_raster(dom, node);
                if let Some(source) = visual::raster_source_point(dom, node, intrinsic.0, intrinsic.1, px, py) {
                    let pixel = sample_drawable_pixel(io, &json!(node), intrinsic, source.0, source.1)?;
                    let sample = visual::raster_finish(dom, node, pixel);
                    if is_sampled(&sample) {
                        return Ok(sample);
                    }
                    unresolved.push(sample_reason(&sample));
                }
            }
            _ => {
                let sample = sample_css_background(io, dom, node, px, py, text_color)?;
                if is_sampled(&sample) {
                    if visual::sample_is_opaque(&sample) {
                        return Ok(sample);
                    }
                    let parent = dom.parent(node).or_else(|| dom.body()).unwrap_or(0);
                    let under = sample_background(io, dom, parent, px, py, depth + 1.0, text_color)?;
                    return Ok(visual::alpha_composite(sample, &under));
                }
                unresolved.push(sample_reason(&sample));
            }
        }
    }
    Ok(visual::unresolved_from_reasons(&unresolved))
}

fn analyze_candidate(io: &mut dyn PageIo, dom: &SnapshotDom, candidate: &Value) -> Result<Value, String> {
    let prepared = resolve_needs(dom, io, |d| visual::prepare_analysis(d, candidate))?;
    let (el, points, text_color) = match prepared {
        Prepared::Early { early } => return Ok(early),
        Prepared::Ready { el, points, text_color } => (el, points, text_color),
    };
    let mut samples = Vec::with_capacity(points.len());
    for point in &points {
        let px = point.get("x").and_then(Value::as_f64).unwrap_or(0.0);
        let py = point.get("y").and_then(Value::as_f64).unwrap_or(0.0);
        samples.push(sample_background(io, dom, el, px, py, 0.0, &text_color)?);
    }
    Ok(visual::finish_analysis(candidate, &text_color, &samples, points.len()))
}

/// Candidates from the core, one analysis each; an off-screen candidate is
/// scrolled into view, re-measured, and analyzed again, then the scroll is
/// restored.
fn analyze_visual_contrast(io: &mut dyn PageIo, base: &SnapshotDom) -> Result<Vec<Value>, String> {
    // Image-backed text first (the live overlay's pass), then every other
    // candidate (the URL engine's pass), so gradient-heavy pages cannot crowd
    // image backgrounds out of the cap.
    let image_only = json!({ "maxCandidates": MAX_VISUAL_CANDIDATES, "imageOnly": true });
    let general = json!({ "maxCandidates": MAX_VISUAL_CANDIDATES });
    let mut candidates = resolve_needs(base, io, |d| visual::collect_visual_contrast_candidates(d, &image_only))?;
    for candidate in resolve_needs(base, io, |d| visual::collect_visual_contrast_candidates(d, &general))? {
        let selector = candidate.get("selector");
        if !candidates.iter().any(|c| c.get("selector") == selector) {
            candidates.push(candidate);
        }
    }
    let restore = live_scroll(io)?;
    let mut results = Vec::with_capacity(candidates.len());
    for candidate in &candidates {
        if live_scroll(io)? != restore {
            io.call(json!({ "op": "scrollTo", "x": restore.0, "y": restore.1 }))?;
            io.call(json!({ "op": "paint" }))?;
        }
        let mut result = analyze_candidate(io, base, candidate)?;
        if visual::needs_scroll_retry(&result) {
            let selector = str_of(candidate.get("selector"));
            if io.call(json!({ "op": "scrollIntoView", "selector": selector }))?.as_bool() == Some(true) {
                io.call(json!({ "op": "paint" }))?;
                let scrolled = recapture_geometry(io, base)?;
                result = analyze_candidate(io, &scrolled, candidate)?;
            }
        }
        results.push(result);
    }
    if live_scroll(io)? != restore {
        io.call(json!({ "op": "scrollTo", "x": restore.0, "y": restore.1 }))?;
    }
    Ok(results)
}

/// Selectors the browser rules already reported as low contrast.
fn reported_low_contrast(groups: &[Value]) -> Vec<&str> {
    groups
        .iter()
        .filter(|g| {
            g.get("findings")
                .and_then(Value::as_array)
                .is_some_and(|fs| fs.iter().any(|f| f.get("type").and_then(Value::as_str) == Some("low-contrast")))
        })
        .filter_map(|g| g.get("selector").and_then(Value::as_str))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Low-contrast findings the analyses decided, minus elements the browser
/// rules already reported.
fn visual_findings(analyses: &[Value], groups: &[Value]) -> Vec<RawResult> {
    let reported = reported_low_contrast(groups);
    analyses
        .iter()
        .filter(|r| {
            r.get("finding").is_some_and(|f| !f.is_null())
                && !r.get("selector").and_then(Value::as_str).is_some_and(|s| reported.contains(&s))
        })
        .filter_map(|r| r.get("finding"))
        .map(|f| RawResult::plain(str_of(f.get("id")), str_of(f.get("snippet"))))
        .collect()
}

// ─── screenshot pixels ─────────────────────────────────────────────────────

/// The pixel fallback for candidates the analyses could not decide (opacity
/// stacks, cross-origin images without CORS, blend modes): screenshot the
/// text, hide it, screenshot again, and compare glyph pixels against what
/// was under them. Ported from the former URL engine
/// (`screenshot-contrast.mjs`), with the clip taken from the live element
/// after scrolling it into view.
fn screenshot_fallback(io: &mut dyn PageIo, analyses: &[Value], groups: &[Value]) -> Result<Vec<RawResult>, String> {
    let reported = reported_low_contrast(groups);
    let open: Vec<&Value> = analyses
        .iter()
        .filter(|a| !matches!(a.get("status").and_then(Value::as_str), Some("fail") | Some("pass")))
        .filter(|a| a.get("selector").and_then(Value::as_str).is_some_and(|s| !s.is_empty() && !reported.contains(&s)))
        .collect();
    if open.is_empty() {
        return Ok(Vec::new());
    }
    let restore = live_scroll(io)?;
    let mut findings = Vec::new();
    for candidate in open {
        if let Some(f) = screenshot_candidate(io, candidate)? {
            findings.push(f);
        }
    }
    io.call(json!({ "op": "scrollTo", "x": restore.0, "y": restore.1 }))?;
    Ok(findings)
}

fn screenshot_candidate(io: &mut dyn PageIo, candidate: &Value) -> Result<Option<RawResult>, String> {
    let selector = str_of(candidate.get("selector"));
    if io.call(json!({ "op": "scrollIntoView", "selector": selector }))?.as_bool() != Some(true) {
        return Ok(None);
    }
    io.call(json!({ "op": "paint" }))?;
    let rect = io.call(json!({ "op": "rect", "selector": selector }))?;
    let Some(clip) = live_clip(&rect) else { return Ok(None) };
    let Some(before) = screenshot(io)? else { return Ok(None) };
    let hide = json!({ "op": "hideText", "selector": selector, "backgroundClipText": candidate.get("backgroundClipText").and_then(Value::as_bool).unwrap_or(false) });
    if io.call(hide)?.as_bool() != Some(true) {
        return Ok(None);
    }
    io.call(json!({ "op": "paint" }))?;
    let after = screenshot(io);
    io.call(json!({ "op": "showText", "selector": selector }))?;
    let Some(after) = after? else { return Ok(None) };
    let viewport_width = rect.get("vw").and_then(Value::as_f64).unwrap_or(0.0);
    let Some(metrics) = compare_contrast(&before, &after, clip, viewport_width, candidate) else { return Ok(None) };
    Ok(pixel_finding(&metrics, candidate))
}

/// The candidate's clip (its rect plus 2 px) through `sanitizeScreenshotClip`,
/// over the element's live viewport rect: whole pixels, inside the viewport,
/// at most 320 px tall.
fn live_clip(rect: &Value) -> Option<[f64; 4]> {
    let n = |k: &str| rect.get(k).and_then(Value::as_f64);
    let (vw, vh) = (n("vw")?, n("vh")?);
    let x = (n("x")? - 2.0).floor().max(0.0);
    let y = (n("y")? - 2.0).floor().max(0.0);
    let right = (n("x")? + n("width")? + 2.0).ceil().min(vw);
    let bottom = (n("y")? + n("height")? + 2.0).ceil().min(vh).min(y + 320.0);
    (right - x >= 1.0 && bottom - y >= 1.0).then_some([x, y, right - x, bottom - y])
}

struct Rgba8 {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

fn screenshot(io: &mut dyn PageIo) -> Result<Option<Rgba8>, String> {
    let out = io.call(json!({ "op": "screenshot" }))?;
    let Some(path) = out.get("path").and_then(Value::as_str) else { return Ok(None) };
    let image = decode_png(std::path::Path::new(path));
    let _ = std::fs::remove_file(path);
    Ok(image)
}

fn decode_png(path: &std::path::Path) -> Option<Rgba8> {
    let file = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let channels = info.color_type.samples();
    let mut px = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        let line = &buf[row * info.line_size..];
        for x in 0..w {
            let s = &line[x * channels..x * channels + channels];
            match channels {
                2 => px.extend_from_slice(&[s[0], s[0], s[0], s[1]]),
                4 => px.extend_from_slice(s),
                _ => return None,
            }
        }
    }
    Some(Rgba8 { w, h, px })
}

struct ContrastMetrics {
    glyph_pixels: usize,
    p10: f64,
    median: f64,
}

/// `compareScreenshotContrast`: pixels that changed when the text was hidden
/// (channel delta of at least 10) are glyph pixels; each pairs the text color
/// (or the rendered pixel) with the background under it. p10 and median over
/// the sorted WCAG ratios; fewer than 8 glyph pixels decide nothing.
fn compare_contrast(before: &Rgba8, after: &Rgba8, clip: [f64; 4], viewport_width: f64, candidate: &Value) -> Option<ContrastMetrics> {
    // Screenshots are in device pixels; the clip is in CSS pixels.
    let scale = if viewport_width > 0.0 { before.w as f64 / viewport_width } else { 1.0 };
    let x0 = (clip[0] * scale).floor() as usize;
    let y0 = (clip[1] * scale).floor() as usize;
    let x1 = (((clip[0] + clip[2]) * scale).ceil() as usize).min(before.w).min(after.w);
    let y1 = (((clip[1] + clip[3]) * scale).ceil() as usize).min(before.h).min(after.h);
    let css_text = match candidate.get("textColor") {
        Some(tc) if tc.is_object() && candidate.get("preferRenderedForeground").and_then(Value::as_bool) != Some(true) => {
            let c = |k: &str| tc.get(k).and_then(Value::as_f64).unwrap_or(0.0);
            Some((c("r"), c("g"), c("b")))
        }
        _ => None,
    };
    let mut glyphs = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let b = &before.px[(y * before.w + x) * 4..][..4];
            let a = &after.px[(y * after.w + x) * 4..][..4];
            let delta: f64 = (0..4).map(|i| (b[i] as f64 - a[i] as f64).abs()).sum();
            if delta < 10.0 {
                continue;
            }
            let fg = css_text.unwrap_or((b[0] as f64, b[1] as f64, b[2] as f64));
            glyphs.push((delta, wcag_ratio(fg, (a[0] as f64, a[1] as f64, a[2] as f64))));
        }
    }
    // With the rendered pixel as foreground, anti-aliased glyph edges are
    // mixes of text and background and would drag p10 down on any text;
    // measure the glyph cores (at least half the strongest change) instead.
    if css_text.is_none() {
        let strongest = glyphs.iter().map(|g| g.0).fold(0.0, f64::max);
        glyphs.retain(|g| g.0 >= strongest * 0.5);
    }
    let mut ratios: Vec<f64> = glyphs.into_iter().map(|g| g.1).collect();
    if ratios.len() < 8 {
        return None;
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pick = |pct: f64| ratios[((pct / 100.0 * ratios.len() as f64).floor() as usize).min(ratios.len() - 1)];
    Some(ContrastMetrics { glyph_pixels: ratios.len(), p10: pick(10.0), median: pick(50.0) })
}

fn wcag_ratio(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let lum = |(r, g, b): (f64, f64, f64)| {
        let ch = |c: f64| {
            let v = c / 255.0;
            if v <= 0.03928 { v / 12.92 } else { impeccino_core::js::math_pow((v + 0.055) / 1.055, 2.4) }
        };
        0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
    };
    let (l1, l2) = (lum(a), lum(b));
    (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05)
}

/// The finding `captureVisualContrastCandidate` reported, when p10 falls
/// under the candidate's threshold.
fn pixel_finding(m: &ContrastMetrics, candidate: &Value) -> Option<RawResult> {
    let threshold = candidate.get("threshold").and_then(Value::as_f64)?;
    if m.glyph_pixels < 8 || !m.p10.is_finite() || m.p10 >= threshold {
        return None;
    }
    let reasons: Vec<String> = candidate
        .get("reasons")
        .and_then(Value::as_array)
        .map(|r| r.iter().take(3).filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    let reason = if reasons.is_empty() { "visual background".to_string() } else { reasons.join(", ") };
    let text = match candidate.get("text").and_then(Value::as_str) {
        Some(t) if !t.is_empty() => format!(" \"{t}\""),
        _ => String::new(),
    };
    let to_fixed = |v: f64| impeccino_core::js::to_fixed(v, 1);
    Some(RawResult::plain(
        "low-contrast",
        format!(
            "pixel contrast {}:1 median {}:1 (need {}:1) on {reason}{text}",
            to_fixed(m.p10),
            to_fixed(m.median),
            impeccino_core::js::number_to_string(threshold)
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, rgb: [u8; 3]) -> Rgba8 {
        Rgba8 { w, h, px: (0..w * h).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect() }
    }

    #[test]
    fn pixel_pass_flags_light_text_on_light_pixels_and_passes_dark_text() {
        let candidate = json!({ "threshold": 4.5, "reasons": ["image background"], "text": "Hello", "textColor": null });
        let bg = solid(20, 10, [230, 230, 230]);
        let mut light = solid(20, 10, [230, 230, 230]);
        let mut dark = solid(20, 10, [230, 230, 230]);
        for i in 0..40 {
            light.px[i * 4..i * 4 + 3].copy_from_slice(&[255, 255, 255]);
            dark.px[i * 4..i * 4 + 3].copy_from_slice(&[20, 20, 20]);
        }
        let clip = [0.0, 0.0, 20.0, 10.0];
        let m = compare_contrast(&light, &bg, clip, 20.0, &candidate).unwrap();
        let f = pixel_finding(&m, &candidate).unwrap();
        assert_eq!(f.id, "low-contrast");
        assert!(f.snippet.starts_with("pixel contrast 1.2:1"), "{}", f.snippet);
        assert!(f.snippet.ends_with("(need 4.5:1) on image background \"Hello\""), "{}", f.snippet);
        let m = compare_contrast(&dark, &bg, clip, 20.0, &candidate).unwrap();
        assert!(pixel_finding(&m, &candidate).is_none());
        // Anti-aliased edges (half-way mixes) do not sink dark rendered text.
        let mut edged = solid(20, 10, [230, 230, 230]);
        for i in 0..60 {
            let rgb = if i < 30 { [20, 20, 20] } else { [200, 200, 200] };
            edged.px[i * 4..i * 4 + 3].copy_from_slice(&rgb);
        }
        let m = compare_contrast(&edged, &bg, clip, 20.0, &candidate).unwrap();
        assert!(pixel_finding(&m, &candidate).is_none());
        // Too few changed pixels decide nothing.
        assert!(compare_contrast(&bg, &bg, clip, 20.0, &candidate).is_none());
    }

    #[test]
    fn clip_stays_inside_the_viewport() {
        let rect = json!({ "x": -4.5, "y": 790.2, "width": 2000.0, "height": 500.0, "vw": 1280.0, "vh": 800.0 });
        assert_eq!(live_clip(&rect), Some([0.0, 788.0, 1280.0, 12.0]));
        assert_eq!(live_clip(&json!({ "x": 0, "y": 900, "width": 10, "height": 10, "vw": 1280, "vh": 800 })), None);
    }
}
