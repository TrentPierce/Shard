# Website Audit & Improvement Recommendations

Date: 2026-02-21  
Scope: `web/` (Next.js application), with emphasis on UX, accessibility, performance, and security posture.

## Executive summary

Shard already has a strong visual identity and several good platform fundamentals (metadata, responsive layout, security headers/CSP, and deterministic app shell structure). The highest-impact opportunities are:

1. **Accessibility hardening** (landmarks, focus states consistency, color contrast verification, and keyboard affordances).
2. **Performance optimization** (font delivery strategy, client-heavy rendering split, and telemetry polling discipline).
3. **Security tightening** (reduce CSP permissiveness, especially `unsafe-inline`/`unsafe-eval` and broad `connect-src *`).
4. **Content/SEO strengthening** (social preview image, richer structured data, and clearer information scent on landing + app surfaces).

---

## What was reviewed

- App shell and metadata: `web/src/app/layout.tsx`
- Main experience composition: `web/src/app/page.tsx`
- Landing experience copy/layout: `web/src/components/LandingPage.tsx`
- Header/navigation behavior: `web/src/components/Header.tsx`
- Chat interaction flow: `web/src/components/ChatPanel.tsx`
- Global styling + responsive rules: `web/src/app/globals.css`
- Runtime security middleware and headers: `web/src/middleware.ts`
- Next.js config and security header injection: `web/next.config.js`

---

## Strengths observed

- **Thoughtful metadata baseline** (canonical, OpenGraph/Twitter text, manifest, viewport tuning).
- **Clear product narrative** in landing sections (pillars + role cards).
- **Reasonable default security controls** (`X-Frame-Options`, `nosniff`, HSTS, Permissions-Policy, CSP present).
- **Responsive grid behavior** already present for app shell and content columns.
- **Useful realtime UX cues** in chat/network states.

---

## Recommendations by priority

## P0 — Address next

### 1) Tighten CSP and script execution model
**Why it matters:** Current CSP includes broad allowances (`'unsafe-inline'`, `'unsafe-eval'`, and wildcard network destinations). This reduces protection against script injection and data exfil vectors.

**Recommendations:**
- Replace inline script registration blocks with nonce-based `<Script>` usage or first-party module scripts.
- Remove `'unsafe-eval'` where possible (or scope it to development only).
- Narrow `connect-src` to explicit origins/environment-configured allowlists instead of `*`.
- Ensure the generated nonce is actually consumed by scripts that need execution.

**Expected impact:** Higher resilience to XSS and stronger compliance posture.

### 2) Accessibility pass on interactive patterns
**Why it matters:** The UI is visually strong, but dense custom styling and dynamic panels can regress keyboard/screen-reader usability.

**Recommendations:**
- Verify every actionable control has a visible and consistent focus style.
- Add/validate semantic landmarks (`header`, `nav`, `main`, `section`, `aside`) and heading hierarchy on all routes.
- Audit contrast for chips/badges in both light and dark themes.
- Add aria-live strategy review for streaming chat updates to avoid over-announcement.

**Expected impact:** Better usability, improved compliance, and stronger retention for keyboard-only users.

---

## P1 — High value

### 3) Improve first-load performance and rendering split
**Why it matters:** The home page is client-heavy and imports multiple real-time modules. This can increase hydration cost and time-to-interactive.

**Recommendations:**
- Move non-interactive shell elements to server components where feasible.
- Lazy-load high-cost visual modules (e.g., visualizers/charts) behind visibility/interaction gates.
- Reduce polling pressure (backoff when tab hidden; consolidate polling endpoints).
- Add bundle analysis (`next build --analyze`) and set performance budgets.

**Expected impact:** Faster load, better Core Web Vitals, lower battery/CPU usage.

### 4) Font loading strategy optimization
**Why it matters:** Multiple Google Font families/weights are fetched; this can increase connection/setup and render cost.

**Recommendations:**
- Prefer `next/font` for self-hosted, subsetted fonts.
- Trim unused weights and families.
- Preload only critical typography for above-the-fold content.

**Expected impact:** Lower CLS/FCP risk and more predictable rendering.

### 5) SEO/social discoverability enhancements
**Why it matters:** Metadata text is good, but social previews and structured context can be strengthened.

**Recommendations:**
- Add explicit OG/Twitter image assets and dimensions.
- Add JSON-LD structured data (Organization + SoftwareApplication).
- Add route-specific titles/descriptions for `/network` and `/dashboard`.
- Validate canonical strategy if alternate deployment domains are used.

**Expected impact:** Better link previews and improved search clarity.

---

## P2 — Nice-to-have / iterative

### 6) Product analytics + UX instrumentation
- Track prompt-to-first-token and prompt-to-complete timings in the client.
- Track quick-prompt usage and abandonment points.
- Add feature flags for experimental interaction modes.

### 7) Design system consistency cleanup
- Reduce inline style usage in components in favor of tokens/classes.
- Consolidate repeated nav definitions to one source of truth.
- Add a small component accessibility checklist to PR templates.

### 8) Operational readiness polish
- Add synthetic uptime checks for primary routes.
- Add web-vitals reporting to backend observability.
- Verify fallback UX for API outage and topology endpoint timeout.

---

## Suggested 2-week implementation plan

### Week 1
- CSP hardening pass + nonce wiring cleanup.
- Accessibility audit (keyboard path + contrast scan + landmark/headings check).
- Font migration to `next/font` and weight reduction.

### Week 2
- Rendering split and lazy-loading of heavy modules.
- Route-level metadata + OG image pipeline.
- Introduce perf/UX dashboards (TTFB, FCP, LCP, interaction timings).

---

## Validation checklist after implementation

- Run Lighthouse (mobile + desktop) and compare before/after.
- Run axe accessibility checks on `/`, `/network`, `/dashboard`.
- Confirm CSP still allows required runtime flows in browser + Tauri builds.
- Verify no regressions in chat streaming behavior.
- Track Core Web Vitals over at least one release cycle.

