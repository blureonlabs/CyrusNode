---
key: <industry_key>
display_name: <Human-readable name>
locale_tone: en_AE
---

# Industry: <name>

> Template for industry playbooks. Copy to `<industry_key>.md` and fill out. The prompt loader injects relevant sections into prompts.

## Typical pains
- Bullet of common operational pain points in this industry.
- Each pain should be specific enough to cite as evidence (e.g. "high call abandonment after hours").

## Typical opportunities
- Subset of catalogue ids that usually fit. The Opportunity Finder will still need evidence; this is a prior, not a guarantee.
- Format: `- <opportunity_id> — why it usually fits`.

## Baselines (for ROI Estimator)
- Average appointment / customer / order value.
- Conversion rate of inquiry → customer.
- Typical inquiry volume signal (e.g. "good clinic with 4.5★ and 200+ reviews receives ~10 WhatsApp inquiries/day").
- Uplift typically observed from each opportunity (be conservative).

## Tone (for Email Writer + WhatsApp Writer)
- 2–3 sentences describing the voice.
- What words to use. What words to avoid.
- Example: "Direct, peer-to-peer. Use specific service names (cosmetic, Invisalign). Avoid 'revolutionary', 'cutting-edge'."

## Banned for this industry
- Words or claims that are specifically inappropriate here (e.g. medical exaggerations for clinics).

## Reference cases (will grow over time)
- After wins, paste anonymized one-liners here. The Meeting Coach also pulls from Qdrant but reading this is useful for the operator.

Links: [[../ai-services]] · [[../../03-agents/opportunity-finder]]
