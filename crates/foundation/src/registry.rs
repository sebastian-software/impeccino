//! Built-in rule metadata and the append-only rule-pack registry.
//! Registered rows resolve after built-ins and cannot shadow an existing id.

use std::sync::{OnceLock, RwLock};

/// Metadata carried by a built-in or registered rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Antipattern {
    pub id: &'static str,
    pub category: &'static str,
    /// JS `scopes` (e.g. `['type']`).
    pub scopes: Option<&'static [&'static str]>,
    /// JS `severity` (`'error'`, `'advisory'`); `finding()` defaults it to
    /// `'warning'` when absent.
    pub severity: Option<&'static str>,
    pub name: &'static str,
    pub description: &'static str,
}

/// JS `ANTIPATTERNS`, in registry order.
pub static ANTIPATTERNS: &[Antipattern] = &[
    Antipattern {
        id: "side-tab",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Side-tab accent border",
        description: "Thick colored border on one side of a card — the most recognizable tell of AI-generated UIs. Use a subtler accent or remove it entirely.",
    },
    Antipattern {
        id: "border-accent-on-rounded",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Border accent on rounded element",
        description: "Thick accent border on a rounded card — the border clashes with the rounded corners. Remove the border or the border-radius.",
    },
    Antipattern {
        id: "overused-font",
        category: "slop",
        scopes: Some(&["type"]),
        severity: None,
        name: "Overused font",
        description: "Inter, Roboto, Fraunces, Geist, Plus Jakarta Sans, and Space Grotesk are used on so many sites they no longer feel distinctive. Each new wave of AI-generated UIs converges on the same handful of faces. Choose a face that gives your interface personality.",
    },
    Antipattern {
        id: "flat-type-hierarchy",
        category: "slop",
        scopes: Some(&["type"]),
        severity: None,
        name: "Flat type hierarchy",
        description: "Dominant heading and body roles are separated by less than 1.25× at every step, leaving the size hierarchy flat. Add at least one stronger size step.",
    },
    Antipattern {
        id: "gradient-text",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Gradient text",
        description: "Gradient text is decorative rather than meaningful — a common AI tell, especially on headings and metrics. Use solid colors for text.",
    },
    Antipattern {
        id: "ai-color-palette",
        category: "slop",
        scopes: None,
        severity: None,
        name: "AI color palette",
        description: "Purple/violet gradients and cyan-on-dark are the most recognizable tells of AI-generated UIs. A gradient in one of those hues is the tell on its own; flat neon ink on a dark ground is charged once a second tell hue joins it. Choose a distinctive, intentional palette.",
    },
    Antipattern {
        id: "cream-palette",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Cream / beige palette",
        description: "A warm cream or beige page background has become the default \"tasteful\" AI surface, reached for by reflex. Choose a background that comes from a deliberate palette, not the safe warm off-white.",
    },
    Antipattern {
        id: "nested-cards",
        category: "slop",
        scopes: Some(&["layout"]),
        severity: None,
        name: "Nested cards",
        description: "Cards inside cards create visual noise and excessive depth. Flatten the hierarchy — use spacing, typography, and dividers instead of nesting containers.",
    },
    Antipattern {
        id: "monotonous-spacing",
        category: "slop",
        scopes: Some(&["layout"]),
        severity: None,
        name: "Monotonous spacing",
        description: "The same spacing value used everywhere — no rhythm, no variation. Use tight groupings for related items and generous separations between sections.",
    },
    Antipattern {
        id: "bounce-easing",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Bounce or elastic easing",
        description: "Bounce and elastic easing feel dated and tacky. Real objects decelerate smoothly — use exponential easing (ease-out-quart/quint/expo) instead.",
    },
    Antipattern {
        id: "pulsing-dot",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Pulsing status dot",
        description: "Small pulsing status dots simulate liveness decoratively. Reserve pulse animation for indicators tied to genuinely live, changing data; a static indicator with clear labeling is honest and calmer.",
    },
    Antipattern {
        id: "blinking-cursor",
        category: "slop",
        scopes: None,
        severity: Some("advisory"),
        name: "Decorative blinking cursor",
        description: "A blinking text cursor animated into a hero or landing section simulates typing where no input exists. It borrows the dev-tool aesthetic as decoration. Real editable fields draw their own caret; anywhere else, let the composition hold attention without a fake prompt.",
    },
    Antipattern {
        id: "shape-assembled-illustration",
        category: "slop",
        scopes: None,
        severity: Some("advisory"),
        name: "Shape-assembled illustration",
        description: "A large inline SVG that builds a pictorial scene from a pile of primitive shapes reads as placeholder clip art, not illustration. Icons, logos, and data graphics are fine at their scale; a hero-sized visual deserves real artwork, a photograph, or a deliberately drawn graphic.",
    },
    Antipattern {
        id: "organic-clip-path",
        category: "quality",
        scopes: None,
        severity: None,
        name: "Organic contour drawn as clip-path",
        description: "A clip-path polygon with many arbitrary vertices, or a curved clip-path path(), is CSS approximating a torn edge, blob, or silhouette. It reads as the cheap version of the effect and is usually a produced or photographic material replaced with code. Derive an alpha matte from the real image, or ship the shape as a cut-out raster; keep clip-path for geometry (cut corners, diagonals, hexagons).",
    },
    Antipattern {
        id: "buried-raster",
        category: "quality",
        scopes: None,
        severity: None,
        name: "Raster buried under a wash or opacity",
        description: "A background image under a near-opaque gradient wash, or a raster on an element at near-zero opacity, never reaches the screen: the page shows the wash, and the produced texture or photo ships as a compliance token. Let the material show (a tint under 0.9 alpha, a blend mode, an opacity you can see) or remove the file.",
    },
    Antipattern {
        id: "dark-glow",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Glowing shadow accents",
        description: "Colored glow shadows — a zero-offset chromatic halo (box- or text-shadow) on any background, or any colored blurred shadow on a dark background — are the default \"cool\" look of AI-generated UIs. Use neutral elevation shadows and subtle, purposeful lighting instead.",
    },
    Antipattern {
        id: "radial-halo",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Radial-gradient background halo",
        description: "A chromatic radial-gradient wash — saturated at the center, fading to transparent — used as a decorative background glow on a dark page. Same tell as glowing shadows, drawn with a gradient instead of a shadow. Ground the surface with a solid or subtly shifted background instead.",
    },
    Antipattern {
        id: "radial-spotlight-glow",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Decorative radial spotlight glow",
        description: "A soft, low-opacity accent-colored radial gradient fading to transparent, dropped behind a hero or section as a \"spotlight.\" It is a reflex AI decoration — the translucent cousin of the saturated radial halo. Let the surface stand on its own, or light the composition with a deliberate material accent rather than a floating colored haze.",
    },
    Antipattern {
        id: "marquee",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Auto-scrolling marquee",
        description: "Continuously auto-scrolling content demands attention it has not earned and hides half its content at any moment. Reserve motion for content that changes; let readers move at their own pace.",
    },
    Antipattern {
        id: "icon-tile-stack",
        category: "slop",
        scopes: Some(&["layout"]),
        severity: None,
        name: "Icon tile stacked above heading",
        description: "A small rounded-square icon container above a heading is the universal AI feature-card template — every generator outputs this exact shape. Try a side-by-side icon and heading, or let the icon sit in flow without its own container.",
    },
    Antipattern {
        id: "italic-serif-display",
        category: "slop",
        scopes: Some(&["type"]),
        severity: None,
        name: "Italic serif display headline",
        description: "Oversized italic serif (Fraunces, Recoleta, Playfair, Newsreader-italic) as the primary hero headline reads as taste in isolation but has become the universal AI-startup landing page hero. Set roman, or move to a non-serif display face. Editorial / magazine register may legitimately want this — judge by context.",
    },
    Antipattern {
        id: "hero-eyebrow-chip",
        category: "slop",
        scopes: Some(&["type"]),
        severity: None,
        name: "Hero eyebrow / pill chip",
        description: "A tiny uppercase letter-spaced label sitting immediately above an oversized hero headline — or the same shape rendered as a pill chip — is now the default AI SaaS hero. Drop the eyebrow, integrate the kicker into the headline, or run it as a navigation breadcrumb instead.",
    },
    Antipattern {
        id: "kicker-above-heading",
        category: "slop",
        scopes: Some(&["type"]),
        severity: None,
        name: "Kicker / eyebrow label above heading",
        description: "A tiny tracked uppercase or small-caps label sitting as its own block directly above a heading is banned outright, repeated or not. Generated kickers never earn their place: the heading carries its own weight. Delete the label and let the heading speak; if the words matter, work them into the heading or the body.",
    },
    Antipattern {
        id: "numbered-section-labels",
        category: "slop",
        scopes: Some(&["type"]),
        severity: Some("advisory"),
        name: "Tiny numbered section labels",
        description: "Small numeric index labels riding next to section headings, repeated section after section, are AI editorial scaffolding — a page numbering its own chapters instead of earning structure. Let hierarchy, content, and rhythm carry the sequence.",
    },
    Antipattern {
        id: "em-dash-overuse",
        category: "slop",
        scopes: None,
        severity: Some("advisory"),
        name: "Em-dash overuse",
        description: "Em-dash saturation in body copy is an AI cadence tell. Advisory only: humans use em-dashes legitimately, so this fires only on saturation — at least 8 em-dashes (— or --) at a density near one per 500 characters of body text — never on a long article that uses a few. Prefer commas, colons, periods, or parentheses.",
    },
    Antipattern {
        id: "marketing-buzzword",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Marketing buzzword",
        description: "Generic SaaS phrases (streamline / empower / supercharge / world-class / enterprise-grade / next-generation / cutting-edge / etc) are instant AI tells. Pick a specific verb and noun that says what the product literally does.",
    },
    Antipattern {
        id: "aphoristic-cadence",
        category: "slop",
        scopes: None,
        severity: None,
        name: "Aphoristic-cadence copy",
        description: "Three or more sections landing on a short rebuttal sentence (\"X. No Y.\" / \"X. Just Y.\") or a manufactured-contrast aphorism (\"Not a feature. A platform.\") reads as AI cadence, not voice. Once is fine; the pattern is the tell.",
    },
    Antipattern {
        id: "oversized-h1",
        category: "slop",
        scopes: Some(&["type"]),
        severity: None,
        name: "Oversized hero headline",
        description: "A full-sentence headline set at display size ends up dominating the viewport, leaving no room for anything else above the fold. A punchy one- or two-word headline at that size is fine — the problem is a long headline blown up too large. Set long headlines smaller, or tighten the copy.",
    },
    Antipattern {
        id: "extreme-negative-tracking",
        category: "slop",
        scopes: Some(&["type"]),
        severity: None,
        name: "Crushed letter spacing",
        description: "Letter-spacing pulled tighter than the point where characters keep their own shapes costs legibility. Tighten display type optically, not destructively.",
    },
    Antipattern {
        id: "broken-image",
        category: "quality",
        scopes: None,
        severity: None,
        name: "Broken or placeholder image",
        description: "<img> tags with empty src, missing src, or placeholder values ship as broken-image boxes. Use real images, generated assets, or remove the tag.",
    },
    Antipattern {
        id: "script-error",
        category: "quality",
        scopes: None,
        severity: Some("error"),
        name: "Uncaught script error on load",
        description: "A script threw an uncaught exception or failed to parse while the page loaded. Broken JavaScript silently kills reveals, interactions, and dynamic content, and can leave most of a page invisible. Fix the error before judging anything else.",
    },
    Antipattern {
        id: "content-hidden-at-rest",
        category: "quality",
        scopes: Some(&["layout"]),
        severity: Some("error"),
        name: "Content invisible at rest",
        description: "A large share of the page text sits at opacity 0 or visibility hidden even after every reveal handler had a chance to run. This is the failed-reveal signature: the content shipped but never becomes visible. Make content visible by default and let JavaScript enhance its entrance instead of gating its existence.",
    },
    Antipattern {
        id: "edge-flush-cards",
        category: "quality",
        scopes: Some(&["layout"]),
        severity: None,
        name: "Cards flush against the scroller edge",
        description: "Cards inside a horizontal scroller or tab panel sit flush against the container edge at rest while keeping a gutter on the other side, so their edges and rounded corners get cut off. Usually the panel is sized wider than its clip box. Keep a consistent inset on both sides.",
    },
    Antipattern {
        id: "text-occlusion",
        category: "quality",
        scopes: Some(&["layout"]),
        severity: None,
        name: "Text occluded by an overlapping element",
        description: "Text is painted under an opaque element or a second text run, so part of it cannot be read. A decorative box, a stacked layer, or an inline element with leaked padding lands on the words instead of beside them. Give overlapping layers room, or move the text out from under the layer above it.",
    },
    Antipattern {
        id: "first-viewport-column-overflow",
        category: "quality",
        scopes: Some(&["layout"]),
        severity: None,
        name: "One column stretches the first viewport",
        description: "A multi-column opening section lets one column run far past the fold while its sibling fits in a single viewport, so the short column floats in dead space and the fold falls deep inside one section. Balance the columns, cap the tall one, or let the long content flow below the opening row.",
    },
    Antipattern {
        id: "gray-on-color",
        category: "quality",
        scopes: None,
        severity: None,
        name: "Gray text on colored background",
        description: "Gray text looks washed out on colored backgrounds. Use a darker shade of the background color instead, or white/near-white for contrast.",
    },
    Antipattern {
        id: "low-contrast",
        category: "quality",
        scopes: None,
        severity: None,
        name: "Low contrast text",
        description: "Text does not meet WCAG AA contrast requirements (4.5:1 for body, 3:1 for large text). Increase the contrast between text and background.",
    },
    Antipattern {
        id: "layout-transition",
        category: "quality",
        scopes: None,
        severity: None,
        name: "Layout property animation",
        description: "Animating width, height, padding, or margin causes layout thrash and janky performance. Use transform and opacity instead, or grid-template-rows for height animations.",
    },
    Antipattern {
        id: "line-length",
        category: "quality",
        scopes: Some(&["type", "layout"]),
        severity: None,
        name: "Line length too long",
        description: "Text lines wider than ~80 characters are hard to read. The eye loses its place tracking back to the start of the next line, so it is measured on the lines that rendered and charged when more than one of them runs long. Add a max-width (65ch to 75ch) to text containers.",
    },
    Antipattern {
        id: "cramped-padding",
        category: "quality",
        scopes: Some(&["layout"]),
        severity: None,
        name: "Cramped padding",
        description: "Text is too close to the edge of its container. Two shapes: (1) an element with its own text where the space between the rendered text and the border box is too small for the font size, and (2) a wrapper whose children's text lands flush against a visible boundary (border, outline, or non-transparent background) with nothing to inset it. Add at least 8px (ideally 12–16px) of space inside bordered, outlined, or colored containers.",
    },
    Antipattern {
        id: "body-text-viewport-edge",
        category: "quality",
        scopes: Some(&["layout"]),
        severity: None,
        name: "Body text touching viewport edge",
        description: "Body paragraphs render flush against the left or right viewport edge with no container providing horizontal padding. Wrap content in a container with at least 16px (ideally 24-32px) of horizontal padding, or apply max-width with mx-auto.",
    },
    Antipattern {
        id: "tight-leading",
        category: "quality",
        scopes: Some(&["type"]),
        severity: None,
        name: "Tight line height",
        description: "Line height below 1.3x the font size makes multi-line text hard to read. Use 1.5 to 1.7 for body text so lines have room to breathe.",
    },
    Antipattern {
        id: "skipped-heading",
        category: "quality",
        scopes: Some(&["type"]),
        severity: None,
        name: "Skipped heading level",
        description: "Heading levels should not skip (e.g. h1 then h3 with no h2). Screen readers use heading hierarchy for navigation. Skipping levels breaks the document outline.",
    },
    Antipattern {
        id: "heading-rhythm",
        category: "quality",
        scopes: Some(&["layout", "type"]),
        severity: None,
        name: "Heading crowded against the previous block",
        description: "A heading binds to the content it introduces, so the rendered space above it should exceed the space below it. When headings across a page sit as close or closer to the block above than to their own content, every section reads as if it captions the previous one. Open up the space above each heading.",
    },
    Antipattern {
        id: "justified-text",
        category: "quality",
        scopes: Some(&["type"]),
        severity: None,
        name: "Justified text",
        description: "Justified text without hyphenation creates uneven word spacing (\"rivers of white\"). Use text-align: left for body text, or enable hyphens: auto if you must justify.",
    },
    Antipattern {
        id: "tiny-text",
        category: "quality",
        scopes: Some(&["type"]),
        severity: None,
        name: "Tiny body text",
        description: "Body text below 12px is hard to read, especially on high-DPI screens. Use at least 14px for body content, 16px is ideal.",
    },
    Antipattern {
        id: "undersized-ui-text",
        category: "quality",
        scopes: Some(&["type"]),
        severity: None,
        name: "Undersized functional text",
        description: "Interactive and content-bearing UI text (links, buttons, nav items, labels, table cells, meta rows, timecodes) below 11px is a legibility failure, not a style choice. WCAG sets no absolute pixel floor, but functional text under 11px is a defensible quality bar: it fails on high-DPI and small viewports and it degrades tap and read targets. The 11px floor holds even inside a footer; only non-interactive legal smallprint gets the softer 10px floor. Being ON the DESIGN.md size ramp does not exempt a value here: adding 8px to the ramp launders the token but not the legibility problem, and that is exactly the escape hatch this rule closes. Exempts sup/sub, visually-hidden (sr-only) text, and code/terminal contexts. Decorative letterspaced micro-labels are still functional and stay in scope.",
    },
    Antipattern {
        id: "all-caps-body",
        category: "quality",
        scopes: Some(&["type"]),
        severity: None,
        name: "All-caps body text",
        description: "Long passages in uppercase are hard to read. We recognize words by shape (ascenders and descenders), which all-caps removes. Reserve uppercase for short labels and headings.",
    },
    Antipattern {
        id: "wide-tracking",
        category: "quality",
        scopes: Some(&["type"]),
        severity: None,
        name: "Wide letter spacing on body text",
        description: "Letter spacing above 0.05em on body text disrupts natural character groupings and slows reading. Reserve wide tracking for short uppercase labels only.",
    },
    Antipattern {
        id: "text-overflow",
        category: "quality",
        scopes: Some(&["layout"]),
        severity: None,
        name: "Content overflowing its container",
        description: "Content renders wider than its container, spilling out or forcing a horizontal scrollbar. Let text wrap, constrain widths, or give the region a deliberate scroll affordance.",
    },
    Antipattern {
        id: "repeated-container-text",
        category: "quality",
        scopes: None,
        severity: None,
        name: "Same text repeated inside one container",
        description: "The same literal text rendered three or more times in structurally different spots inside a single card or panel is redundant messaging — usually a status or label wired into every slot of a template. Say it once, in the slot where it matters most.",
    },
    Antipattern {
        id: "clipped-overflow-container",
        category: "quality",
        scopes: Some(&["layout"]),
        severity: None,
        name: "Positioned child clipped by overflow container",
        description: "A clipping container (overflow hidden or clip) wrapping an absolutely-positioned child cuts off tooltips, menus, and popovers that need to escape. Let the overflow be visible, or move the positioned layer out of the clip.",
    },
    Antipattern {
        id: "design-system-font",
        category: "quality",
        scopes: Some(&["type"]),
        severity: None,
        name: "Font outside DESIGN.md",
        description: "A font is used that is not declared in DESIGN.md typography. Use the documented type system or update DESIGN.md if this is an intentional brand addition.",
    },
    Antipattern {
        id: "design-system-color",
        category: "quality",
        scopes: None,
        severity: Some("advisory"),
        name: "Color outside DESIGN.md",
        description: "A literal color is outside the DESIGN.md palette and sidecar tonal ramps. This may be legitimate, but it should be an intentional design-system addition rather than drift.",
    },
    Antipattern {
        id: "design-system-radius",
        category: "quality",
        scopes: None,
        severity: Some("advisory"),
        name: "Radius outside DESIGN.md",
        description: "A border-radius value is outside the DESIGN.md rounded scale. Use a documented radius token or update the design system if the new shape is intentional.",
    },
    Antipattern {
        id: "design-system-font-size",
        category: "quality",
        scopes: Some(&["type"]),
        severity: Some("advisory"),
        name: "Font size outside DESIGN.md",
        description: "A literal font-size is off the type ramp documented in DESIGN.md typography. Use a documented size step or update the design system if the new step is intentional.",
    },
    Antipattern {
        id: "gpt-thin-border-wide-shadow",
        category: "slop",
        scopes: None,
        severity: Some("advisory"),
        name: "Hairline border with wide shadow",
        description: "A hairline border paired with a wide, diffuse shadow is a recurring generated-UI signature. Commit to one — a defined edge or a soft elevation — rather than both at once.",
    },
    Antipattern {
        id: "repeating-stripes-gradient",
        category: "slop",
        scopes: None,
        severity: Some("advisory"),
        name: "Repeating-gradient stripes",
        description: "Repeating-gradient stripes used as surface decoration are a recurring generated-UI signature. Reach for a deliberate texture or leave the surface plain.",
    },
    Antipattern {
        id: "codex-grid-background",
        category: "slop",
        scopes: None,
        severity: Some("advisory"),
        name: "Decorative grid-line background",
        description: "A decorative grid or line-field background drawn with hairline linear-gradient layers tiled by a fixed pixel cell is a recurring generated-UI signature. Reserve grid overlays for actual canvas, map, blueprint, or measurement surfaces; elsewhere use product structure or a plain surface.",
    },
    Antipattern {
        id: "theater-slop-phrase",
        category: "slop",
        scopes: None,
        severity: Some("advisory"),
        name: "Theater framing copy",
        description: "Dismissing something as \"theater\" is a recurring generated-copy tic. Say plainly what the thing does or does not do.",
    },
    Antipattern {
        id: "image-hover-transform",
        category: "slop",
        scopes: None,
        severity: Some("advisory"),
        name: "Image hover transform",
        description: "Scaling or rotating an image on hover is a recurring generated-UI signature. Let imagery sit still, or use a subtler, purposeful interaction.",
    },
];

/// The rules the design hook fixes at edit time rather than deferring to a
/// review pass: broken output, objective legibility failures, single-property
/// mechanical slop, and design-system drift. Every one of them is mechanical,
/// unambiguous, and cheap to correct at the edit site.
///
/// It lives with the shared registry so the hook can consume it without
/// duplicating its rule list. `hook_lib` re-exports it.
pub const IMMEDIATE_TIER_RULES: &[&str] = &[
    // Broken output.
    "broken-image",
    "text-overflow",
    "clipped-overflow-container",
    "body-text-viewport-edge",
    // Objective contrast / legibility failures.
    "low-contrast",
    "gray-on-color",
    "tiny-text",
    // Single-property mechanical slop, trivial to fix at the edit site.
    "gradient-text",
    "dark-glow",
    // Design-system drift compounds if not corrected at edit time.
    "design-system-font",
    "design-system-color",
    "design-system-radius",
    "design-system-font-size",
];

/// Rows registered by rule packs, in registration order. One entry per
/// `extend` call; `&'static` all the way down, so a lookup can hand out
/// `&'static Antipattern` without holding the lock.
static EXTRA_ROWS: OnceLock<RwLock<Vec<&'static [Antipattern]>>> = OnceLock::new();

fn extra_rows() -> &'static RwLock<Vec<&'static [Antipattern]>> {
    EXTRA_ROWS.get_or_init(|| RwLock::new(Vec::new()))
}

/// A snapshot of the registered slices. Cheap when nothing is registered (an
/// empty `Vec` does not allocate), which is every built-in build.
fn extra_slices() -> Vec<&'static [Antipattern]> {
    match extra_rows().read() {
        Ok(rows) => rows.clone(),
        // The list is append-only `&'static` rows, so a lock poisoned by a
        // panic elsewhere is still sound to read; ignoring the poison keeps a
        // rejected `extend` from making every later lookup miss the rows that
        // did register.
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

/// Register a rule pack's rows. Every registry lookup then resolves them
/// after the built-ins. Calling it again with the same slice is a no-op, so a
/// pack that installs itself from more than one entry point is safe.
///
/// Callers register at startup, before any scan. There is no way to
/// unregister: a rule pack is a property of the process, not of a run.
///
/// # Panics
/// When a row's id collides with a built-in id or with a row another pack
/// already registered. A duplicate id would make `get_antipattern` answer
/// with whichever row came first, which is not a behavior worth guessing at.
pub fn extend(rows: &'static [Antipattern]) {
    let mut registered = match extra_rows().write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if registered.iter().any(|slice| std::ptr::eq(*slice, rows)) {
        return;
    }
    // Collect the complaint first and panic after the guard is dropped: a
    // panic while holding the write guard would poison the lock, and a
    // rejected registration should leave the registry exactly as it was.
    let mut rejected: Option<String> = None;
    for row in rows {
        if let Some(existing) = ANTIPATTERNS.iter().find(|built_in| built_in.id == row.id) {
            rejected = Some(format!(
                "registry::extend: rule id {:?} collides with the built-in rule {:?}; \
                 namespace pack ids (e.g. \"mypack/{}\")",
                row.id, existing.name, row.id
            ));
            break;
        }
        if registered
            .iter()
            .any(|slice| slice.iter().any(|other| other.id == row.id))
        {
            rejected = Some(format!(
                "registry::extend: rule id {:?} is already registered by another rule pack",
                row.id
            ));
            break;
        }
    }
    if let Some(message) = rejected {
        drop(registered);
        panic!("{message}");
    }
    registered.push(rows);
}

/// Every rule the process knows: the built-ins in registry order, then each
/// pack's rows in registration order.
pub fn all_antipatterns() -> impl Iterator<Item = &'static Antipattern> {
    ANTIPATTERNS
        .iter()
        .chain(extra_slices().into_iter().flat_map(|slice| slice.iter()))
}

/// JS `getAntipattern(id)`, extended with the rule packs' rows.
pub fn get_antipattern(id: &str) -> Option<&'static Antipattern> {
    all_antipatterns().find(|rule| rule.id == id)
}

/// JS `getAP(id)` from `findings.mjs` (an alias of `getAntipattern`).
pub fn get_ap(id: &str) -> Option<&'static Antipattern> {
    get_antipattern(id)
}

/// JS `ADVISORY_RULE_IDS`: ids of rules whose registry severity is
/// `'advisory'`, in registry order. `severity` is the canonical field; the
/// finding serializer derives its `advisory: true` output flag from it (pbakaus/impeccable#709).
pub fn advisory_rule_ids() -> impl Iterator<Item = &'static str> {
    all_antipatterns()
        .filter(|rule| rule.severity == Some("advisory"))
        .map(|rule| rule.id)
}

/// JS `isAdvisoryRule(id)`.
pub fn is_advisory_rule(id: &str) -> bool {
    advisory_rule_ids().any(|r| r == id)
}

/// JS `RULE_SCOPES`: every scope tag declared by any rule, first-seen order.
pub fn rule_scopes() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for rule in all_antipatterns() {
        for scope in rule.scopes.unwrap_or(&[]) {
            if !out.contains(scope) {
                out.push(scope);
            }
        }
    }
    out
}

/// JS `filterByScopes(findings, scopes)`: keep findings whose rule declares
/// at least one requested scope; an empty scope list keeps everything.
pub fn filter_by_scopes<F, G>(findings: Vec<F>, scopes: &[&str], antipattern_of: G) -> Vec<F>
where
    G: Fn(&F) -> &str,
{
    if scopes.is_empty() {
        return findings;
    }
    findings
        .into_iter()
        .filter(|f| {
            get_antipattern(antipattern_of(f))
                .and_then(|rule| rule.scopes)
                .unwrap_or(&[])
                .iter()
                .any(|scope| scopes.contains(scope))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_shape() {
        assert_eq!(ANTIPATTERNS.len(), 61);
        assert_eq!(ANTIPATTERNS[0].id, "side-tab");
        assert_eq!(rule_scopes(), vec!["type", "layout"]);
        assert!(is_advisory_rule("em-dash-overuse"));
        // pbakaus/impeccable#709: severity is the canonical advisory field, so every
        // `severity: "advisory"` rule is an advisory rule.
        assert!(is_advisory_rule("blinking-cursor"));
        assert_eq!(
            get_antipattern("blinking-cursor").unwrap().severity,
            Some("advisory")
        );
        let ids: std::collections::HashSet<&str> = ANTIPATTERNS.iter().map(|r| r.id).collect();
        assert_eq!(ids.len(), ANTIPATTERNS.len());
    }

    /// Rows a test pack registers: no scopes and not advisory, so the
    /// built-in assertions in `registry_shape` hold whatever order the test
    /// threads run in.
    static PACK_ROWS: &[Antipattern] = &[
        Antipattern {
            id: "testpack/one",
            category: "quality",
            scopes: None,
            severity: Some("warning"),
            name: "Test pack rule one",
            description: "First row of the registry-extension test pack.",
        },
        Antipattern {
            id: "testpack/two",
            category: "testpack-only",
            scopes: None,
            severity: Some("error"),
            name: "Test pack rule two",
            description: "Second row of the registry-extension test pack.",
        },
    ];

    static COLLIDING_ROWS: &[Antipattern] = &[Antipattern {
        id: "side-tab",
        category: "quality",
        scopes: None,
        severity: None,
        name: "Collides with a built-in",
        description: "Registering this must panic.",
    }];

    #[test]
    fn extension_is_visible_to_every_lookup() {
        assert!(get_antipattern("testpack/one").is_none());
        extend(PACK_ROWS);
        // Idempotent per slice.
        extend(PACK_ROWS);
        extend(PACK_ROWS);

        let one = get_antipattern("testpack/one").expect("pack row resolves");
        assert_eq!(one.name, "Test pack rule one");
        assert_eq!(
            get_ap("testpack/two").map(|r| r.severity),
            Some(Some("error"))
        );
        assert!(get_antipattern("testpack/nope").is_none());

        // Built-ins still come first and are untouched.
        let all: Vec<&str> = all_antipatterns().map(|r| r.id).collect();
        assert_eq!(all.len(), ANTIPATTERNS.len() + PACK_ROWS.len());
        assert_eq!(all[0], "side-tab");
        assert_eq!(
            &all[ANTIPATTERNS.len()..],
            &["testpack/one", "testpack/two"]
        );

        // Pack rows participate in the advisory view.
        assert!(!is_advisory_rule("testpack/one"));

        // A finding built from a pack row carries the pack's metadata.
        let f = crate::findings::finding("testpack/two", "a.tsx", "snip", 3.0);
        assert_eq!(f.name, "Test pack rule two");
        assert_eq!(f.severity, "error");
        assert_eq!(f.category.as_deref(), Some("testpack-only"));
    }

    #[test]
    fn built_in_id_collision_panics() {
        let err = std::panic::catch_unwind(|| extend(COLLIDING_ROWS)).unwrap_err();
        let message = err
            .downcast_ref::<String>()
            .map(String::as_str)
            .unwrap_or("");
        assert!(
            message.contains("collides with the built-in rule"),
            "{message}"
        );
        assert!(message.contains("side-tab"), "{message}");
        assert!(get_antipattern("side-tab")
            .unwrap()
            .name
            .starts_with("Side-tab"));
    }
}
