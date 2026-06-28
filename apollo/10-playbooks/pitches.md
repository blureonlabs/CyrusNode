# Pitches — Apollo and the operator's services

> Two different things, often confused. Apollo is the **internal tool** the
> operator uses to find and write to prospects. The operator sells AI services
> (WhatsApp receptionists, lead-qual agents, operations dashboards) to
> SMBs — *not* Apollo. Don't pitch Apollo to clients.

---

## 1. What Apollo IS (internal description)

> **Apollo is a one-operator AI sales platform that turns a niche keyword into 30 personalized cold outreach drafts in under 10 minutes for less than a dollar.** Tell it "packers and movers in Dubai" and it pulls candidate companies from Google Maps, crawls each one's website, extracts the structured facts (services, fleet, contact info, social presence), runs an LLM-graded business summary against an industry-specific playbook you maintain in Markdown, and writes a human-voiced cold email plus a WhatsApp opener tailored to that one company. It runs locally as a Rust CLI, costs about 1.5 cents per company in LLM bills, and is deliberately unautomated at the send step — the operator reviews every draft, picks the recipient, hits send manually. The thesis is that the moat is not automation but research-grade specificity at scale: a one-person consultancy with Apollo competes against a 20-person AI agency on the quality of its first message, because Apollo does in two minutes what the agency would never bill four hours to do.

**Use this for:** explaining Apollo to engineering peers, writing the project's README, future SaaS landing page, technical talks. **Never send to a prospect.**

---

## 2. What you sell to clients (external pitch)

> I build the AI layer that small operators use to stop losing the bookings
> that come in after hours. For movers, limos, cleaning agencies, clinics
> across the UAE, the same pattern shows up: 30–50% of the inquiry volume
> arrives outside business hours, mostly on WhatsApp, mostly from the
> highest-value customers (villa relocations, weekend bookings, urgent
> service calls). My WhatsApp agent answers in seconds, captures the details
> the way a senior receptionist would, qualifies, and books the survey or
> appointment straight into the operator's calendar. On top of that I stand
> up a live operations dashboard so the owner can see what's happening
> without scrolling WhatsApp — jobs/week, quote-to-survey rate, no-show
> rate, on-time delivery, revenue per crew or per vehicle. Setup is a
> two-week build, then a small monthly retainer. I work with one or two
> operators per industry at a time, so the conversation we have first
> usually decides whether you're that one for your category in Dubai.

**Use this for:** the bottom of your cold emails when someone says "tell me more", proposal docs, your domain's `/about` page, your LinkedIn About section. This is the pitch a prospect should hear.

---

## 3. One-liner — for LinkedIn headline / domain homepage hero

> **I build WhatsApp AI receptionists and operations dashboards for UAE service businesses — movers, limos, clinics, cleaners.**

That's 16 words and it tells a prospect three things: (1) what you build, (2) who it's for, (3) the geography. Nothing else needs to fit in a headline.

---

## 4. Two-line elevator (when someone asks at a coffee)

> "You know how movers and limo companies in Dubai lose half their bookings to missed WhatsApp messages after hours? I build the AI receptionist that catches them, and the dashboard the owner uses to see what's working."

Phrase 1 = the problem they immediately recognise. Phrase 2 = what you do, in plain words. Done. If they're curious they'll ask the next question.

---

## 5. Subject lines that get opened (for cold email)

These are not pitches per se — they're the openers Apollo's email writer is trained to mimic. Operator-tested patterns that work in UAE:

- `quick note on your villa moves`
- `your after-hours WhatsApp`
- `the late-night bookings you're losing`
- `your [specific named service] funnel`
- `a question about [their specific tier]`

Patterns to **avoid** (high spam classification, low open rate):

- "Boost your revenue" / any number-percent claim in subject
- "URGENT" / "ACTION REQUIRED" / brackets of any kind
- "Hi there" / "Hello {first_name}" / template-y openers
- "I help [niche] do X" — too long and too marketer

---

## 6. The honest framing (when a prospect asks "are you using AI?")

> "I am, yeah. The way I work — I research every business before I email them, instead of blasting templates. That research is what AI is doing in the background. What I'm building for you is the same kind of automation, but pointed at your bookings and your customers, not your outbound."

This is the truth (Apollo IS doing AI research; you ARE building AI for them) and it disarms the "is this just AI slop" objection by being upfront that the research is automated *and* personally reviewed by you before it ships.

---

Links: [[sales]] · [[runbook]] · [[../PROJECT_BIBLE]]
