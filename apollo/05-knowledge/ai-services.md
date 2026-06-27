# AI Services Catalogue

> The closed set of services Apollo may propose. The Opportunity Finder is constrained to choose from this list.

Each entry has a stable `id` (used in agent outputs), a one-line description, an effort tier, a typical deliverable, and the kind of signal that should trigger it.

---

## whatsapp-receptionist
- **One-liner**: AI agent that answers WhatsApp 24/7, qualifies the inquiry, and books into the calendar.
- **Effort**: low (2 weeks).
- **Deliverable**: trained agent + handoff rules + calendar integration.
- **Triggers**: business has WhatsApp click-to-chat but no chatbot; volume implied by social or reviews.

## website-chatbot
- **One-liner**: On-site chatbot trained on the company's services and FAQs.
- **Effort**: low.
- **Deliverable**: embedded widget + admin panel for FAQ updates.
- **Triggers**: high-traffic site with no chat; long FAQ pages.

## lead-qualification-agent
- **One-liner**: Conversational agent that qualifies inbound form submissions before they reach sales.
- **Effort**: medium.
- **Deliverable**: agent + CRM integration + qualification rubric.
- **Triggers**: contact form present, no CRM detected, high volume.

## review-automation
- **One-liner**: Auto-request reviews after specific lifecycle events; respond to negative reviews quickly.
- **Effort**: low.
- **Deliverable**: trigger setup + response templates + dashboard.
- **Triggers**: low review count vs. traffic; recent negative reviews unanswered.

## ai-receptionist-voice
- **One-liner**: Voice agent that answers the phone, qualifies, and books appointments.
- **Effort**: medium.
- **Deliverable**: voice number + handoff rules.
- **Triggers**: phone-heavy business; visible after-hours friction.

## ai-content-engine
- **One-liner**: Weekly content pipeline (Instagram + Reels + blog) from a brief.
- **Effort**: medium.
- **Deliverable**: brief template + generation pipeline + approval queue.
- **Triggers**: irregular social cadence; underperforming SEO content.

## internal-rag-knowledge
- **One-liner**: Internal Q&A bot over the company's SOPs, policies, and product docs.
- **Effort**: medium.
- **Deliverable**: ingestion pipeline + Slack/Teams integration.
- **Triggers**: ≥ 30 employees; visible knowledge fragmentation signals.

## image-search-catalog
- **One-liner**: Visual search over a product catalog (e.g. jewellery, fashion, parts).
- **Effort**: medium.
- **Deliverable**: image-embedding pipeline + search UI + admin.
- **Triggers**: large image catalog; users described browsing pain.

## auto-followup-engine
- **One-liner**: Automated personalized follow-ups across email + WhatsApp for stalled leads.
- **Effort**: medium.
- **Deliverable**: trigger setup + content engine + reporting.
- **Triggers**: visible CRM with stalled leads; no marketing-automation tool detected.

## smart-website-rebuild
- **One-liner**: Modern, fast, conversion-focused website rebuild on Next.js (or static if appropriate).
- **Effort**: high.
- **Deliverable**: design + build + content migration + AI-first features baked in.
- **Triggers**: Lighthouse perf < 50; visible conversion friction; outdated CMS.

## seo-audit-fixes
- **One-liner**: Concrete on-page + technical SEO fixes with measurable wins.
- **Effort**: low–medium.
- **Deliverable**: prioritized fix list executed + reporting dashboard.
- **Triggers**: Lighthouse SEO < 70; missing schema; thin content pages.

## geo-audit
- **One-liner**: Track and improve how generative AI engines (ChatGPT, Perplexity, Claude, Gemini, Google AI Overviews) describe the business vs. competitors.
- **Effort**: medium.
- **Deliverable**: visibility report across 3–5 engines + competitor benchmark + prioritized fix list (schema.org, authoritative citations, directory presence).
- **Triggers**: business runs paid ads OR SEO traffic is plateauing OR competitors are visible in AI answers and they aren't.
- **Why it's defensible**: most SMBs don't yet know this is a problem. Apollo's audit is the first time they see it.
- **V1 implementation**: query the business + 2 competitors via 3 engine APIs (OpenAI, Anthropic, Perplexity) with 10 industry-specific queries; parse mentions + citations; output report. Future: add Gemini grounding + scraped Google AI Overviews.

---

## How to add a service
1. Append to this file with the same shape.
2. Update industry playbooks to reference the new `id` where relevant.
3. Re-run Opportunity Finder goldens. Any service id not in this file is rejected at the schema layer.

Links: [[../03-agents/opportunity-finder]] · [[industries/]]
