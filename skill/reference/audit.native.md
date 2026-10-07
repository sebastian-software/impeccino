> Native knowledge curation is pending. The diagnostic material below remains available as source candidates; apply it within task and host instructions.

# Native audit

Review technical quality without editing implementation. Use source, simulator
or device captures, and native input tools for ios, android, or adaptive.
Skip browser tooling and `impeccino detect`, which measure HTML/CSS only.
Read the relevant ios.md/android.md reference(s) when Setup has not supplied them.
Honor recorded style decisions in PRODUCT.md, DESIGN.md, surface briefs, and
relevant ADRs; still report observed accessibility or functional defects and
explain conflicts. Do not infer platform correctness from a screenshot alone.
The report skeleton matches audit.md.

## Diagnostic Scan

Inspect the dimensions relevant to the requested task using actual native evidence.

### 1. Accessibility (VoiceOver / TalkBack)

**Check for**:
- **Missing labels**: interactive elements without accessibility labels, traits/roles, or state announcements
- **Reading and focus order**: illogical traversal, unreachable controls, focus lost on navigation
- **Text scaling**: fixed point sizes defeating Dynamic Type (iOS) or px instead of sp (Android); layouts that clip or overlap at large sizes
- **Touch targets**: below 44 pt (iOS) / 48 dp (Android), or crammed without spacing
- **Reduce Motion ignored**: parallax and large slides with no crossfade alternative
- **Contrast**: text failing contrast in either appearance, light or dark


### 2. Performance

**Check for**:
- **Slow startup**: heavy work on launch before first frame
- **Unvirtualized lists**: long content without FlatList / LazyColumn / List recycling
- **Main-thread jank**: synchronous work in scroll or gesture paths, dropped frames on 60/120 Hz
- **Wasted rendering**: unnecessary re-renders (React Native) or recompositions (Compose); missing memoization/keys
- **Image handling**: full-size images decoded for thumbnails, no caching
- **App weight**: bloated JS bundle or binary, unused dependencies


### 3. Appearance & Theming

**Check for**:
- **Hard-coded colors**: raw hex instead of semantic system colors (iOS) / Material color roles (Android) / design tokens
- **Broken dark appearance**: missing dark variants, poor contrast in dark, quick inverts
- **Dynamic Color** (Android 12+): no static fallback scheme, or ignored where it fits
- **Off-platform materials**: hand-rolled visual materials where system materials or tonal elevation are expected


### 4. Platform Conformance (CRITICAL)

Interpret the loaded platform reference(s) against the task and established system. **Check for**:
- **Broken system gestures**: edge-swipe back disabled (iOS), predictive Back hijacked (Android)
- **Inset violations**: content under the notch, Dynamic Island, home indicator, status bar, or keyboard
- **Off-platform navigation**: custom global nav, overloaded tab bars, iOS patterns on Android or vice versa
- **Web-shaped controls**: HTML-style buttons, custom toggles, hover-dependent affordances
- **Icon drift**: mixed icon sets instead of SF Symbols / Material Symbols
- **System drift**: repeated shortcuts or decorative patterns that conflict with the product, platform, or established design system


### 5. Adaptivity

**Check for**:
- **Stretched phone layouts**: tablet/iPad rendering a scaled-up phone UI instead of using size classes / window size classes
- **Orientation breakage**: landscape clipping, ignored, or locked without reason
- **Keyboard/IME handling**: inputs hidden behind the keyboard, no inset adjustment
- **Multitasking**: iPad Split View / Android multi-window breaking layout
- **Foldables**: hinge-unaware layouts on posture change (Android)



## Generate Report

### Audit Health

Name each assessed dimension as a measured defect, contextual risk, acceptable
within the inspected scope, or unverified. State the method and limits rather
than a numerical health score. A clean scan is not a comprehensive verdict.

### Implementation Integrity

Explain any unsupported claim, missing capability, broken path, or conflict
with recorded decisions. Distinguish observed consequences from assumptions.

### Findings and Actions

Prioritize by user consequence. Each finding names target/location, evidence,
impact, owning decision, and a concrete correction. Include useful strengths and
systemic patterns without treating every familiar style as a defect.

### Coverage

List the paths, viewports/device classes, states, input methods, and tools
actually checked, plus unavailable evidence. Report source versus rendered or
native evidence separately. Screenshots do not verify gestures or complete
accessibility coverage.

## Recommended Actions

Suggest the command that fits each actual finding, such as `/impeccino polish`,
`/impeccino harden`, `/impeccino optimize`, `/impeccino adapt`, or
`/impeccino extract`. The Commands table contains the complete task menu.
An audit alone does not authorize implementation or a documentation rewrite.
Return the report in chat, under the host’s workflow and question policy.
