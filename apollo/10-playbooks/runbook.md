# Operator Runbook — running a real outreach batch

> What you actually do from `git pull` to "30 personalized emails in the operator's outbox" — without leaving the terminal.

This is a checklist, not a tutorial. It assumes you already have `cargo`, a Gemini API key, and (optionally) Resend / Google Maps keys configured.

---

## 0. Prerequisites (one-time, ~10 minutes)

1. Clone + build:
   ```bash
   git clone https://github.com/blureonlabs/CyrusNode
   cd CyrusNode
   cargo build --release --bin apollo-cli
   ```

2. Create `.env` (or `.env.local`) — copy from `.env.local.example`:
   ```env
   GEMINI_API_KEY=AIza...                # Required.
   GOOGLE_MAPS_API_KEY=...               # Required for `apollo discover`.
   RESEND_API_KEY=re_...                 # Optional. Without it, queue dry-runs to .eml files.
   ```

3. Confirm the install:
   ```bash
   ./target/release/apollo-cli industries
   # → dental_clinic (Dental Clinic)
   # → limousine_uae (Limousine / Chauffeur Service (UAE))
   ```

   If you see those, every piece is loaded.

---

## 1. The "$200 limo experiment" — 30-minute version

Goal: 30 personalized cold emails to Dubai limousine companies, queued for your approval.

```bash
# A. Find 30 candidates
apollo-cli discover \
  --industry-query "limousine service Dubai" \
  --city Dubai --country AE --limit 30 \
  | tee out/discover-limos.json \
  | jq -r '.[] | .url // empty' \
  > urls.txt

# B. Research all 30 in parallel
apollo-cli batch urls.txt \
  --industry limousine_uae \
  --signature "Hari" \
  --concurrency 4

# C. See where you stand
apollo-cli status

# D. Check spend
apollo-cli cost --window today --group-by agent
```

**What just happened, in operator terms:**

- `discover` queried Google Places for "limousine service Dubai", filtered to entries with websites, printed a candidate JSON list.
- `batch` opened 4 parallel workers, each running the full pipeline (SSRF-guarded crawl → markdown extract → site facts → BizIntel via Gemini → personalized email via Gemini with the limo playbook tone). Each dossier got saved to `out/dossiers/<uuid>.json`. Auto-detected `info@` / `contact@` emails became recipients.
- `status` shows your queue depth: pending dossiers, outbox files, sent count, today's spend.
- `cost` confirms the bill. Sprint 2 numbers: ~$0.02 per dossier × 30 = $0.60. Sometimes less.

Expected wall-clock: ~6 minutes for the batch (vs ~30 minutes pre-Sprint-2-fix when batch was sequential).

---

## 2. Reviewing — 30 minutes (this is the operator's job)

```bash
apollo-cli queue --from "Hari <hari@example.com>"
```

For each dossier you'll see:

```
URL       : https://www.didilimousine.com/
Industry  : limousine_uae
Summary   : Chauffeur-driven luxury car hire across Dubai...
Recipient : info@didilimousine.com
──────────────────────────────────────────────────────────────
Subject: Ultimate Convenience and Luxury: Next Steps

Your focus on Ultimate convenience and luxury for Dubai clients...
[...]
— Hari

Anchors used: Ultimate convenience and luxury, Easy online booking, Professional drivers
──────────────────────────────────────────────────────────────
[a]pprove  [e]dit  [r]eject  [s]kip  [q]uit:
```

Keys:
- **a** — approve. If `RESEND_API_KEY` is set, sends immediately. Otherwise writes an `.eml` to `out/outbox/` you can drag into Gmail later. If the recipient is missing, you'll be prompted inline.
- **e** — open the body in `$EDITOR` (vim by default; set `EDITOR=code -w` for VS Code). Saved edits persist back to the dossier JSON.
- **r** — reject. Moves the dossier to `out/rejected/`.
- **s** — skip. Leaves it in the queue for next time.
- **q** — quit the review session.

**Aim for ~30 seconds per dossier.** If you're spending more, the playbook tone is off — fix it in `apollo/05-knowledge/industries/<key>.md` and rerun the batch.

---

## 3. The outbox (when Resend isn't set up yet)

Without `RESEND_API_KEY`, approvals write to `out/outbox/dry-run-<timestamp>.eml`. These are valid RFC 822 files:

- Drag-and-drop into Gmail's compose window → it preserves From/To/Subject.
- Or `cat out/outbox/*.eml | mutt` if you prefer.
- Or wire Resend and re-run `apollo queue` — the existing dossiers will be re-processable.

---

## 4. After-send hygiene

Once a week:

```bash
# Drop crawl artifacts older than 7 days (default).
# Always dry-run first if you're not sure.
apollo-cli clean --dry-run
apollo-cli clean

# Aggressive cleanup including processed dossiers.
apollo-cli clean --include-processed
```

`out/dossiers/` (pending review) and `out/outbox/` (waiting for you to send) are **never touched** by clean. Safe to run any time.

---

## 5. Common moves

### "I want to target dental clinics instead of limos"
1. `cp apollo/05-knowledge/industries/limousine_uae.md apollo/05-knowledge/industries/dental_clinic.md`
   (or use the existing `dental_clinic.md` if it's there)
2. Rewrite the `## Tone` section to match how you talk to dentists.
3. `apollo-cli industries` to verify the loader picked it up.
4. `apollo-cli discover --industry-query "dental clinic Dubai" --limit 30 > urls.txt`
5. `apollo-cli batch urls.txt --industry dental_clinic`

### "One company's email came back wrong"
1. Open `out/dossiers/<uuid>.json` in your editor.
2. Either: fix the email body in place, save, re-`apollo queue`.
3. Or: re-run that single URL with `apollo-cli ingest <url> --industry <key>` — overwrites the dossier.

### "What did I spend yesterday?"
```bash
apollo-cli cost --window week --group-by company
# Window filter is rolling 7d; tail -30 the output to see only the most expensive.
```

### "The BizIntel got the industry wrong"
1. The dossier `summary.what_they_sell` will read off.
2. Re-run with `--industry <correct_key>` — playbook hints steer BizIntel.
3. If still wrong, the issue is the prompt (apollo/04-prompts/business-summary.md). Bump version, re-run.

---

## 6. What's NOT here (Sprint 3+)

You're doing these manually until those sprints land:

- **Replies** — when prospects reply to your sent emails, they hit your inbox directly. Apollo doesn't know yet. Sprint 3 (`F-013`) wires Resend's reply webhook + `apollo conversations` to track threads.
- **Proposals** — when someone says "send me a proposal", you write it yourself today. Sprint 3 (`F-009`) wires the Proposal Writer + PDF renderer.
- **Meeting prep** — when a call is booked, you prep manually. Sprint 5 (`F-017`) wires the Meeting Coach + Qdrant retrieval of past wins.

---

## 7. The single most important habit

**After every call (won, lost, or stalled): write down the outcome in a Markdown file under `apollo/11-clients/<host>.md`.**

This is the compounding asset. By client 10, the Meeting Coach (Sprint 5) reads these and serves them as retrieval-augmented context to the LLM when prepping the next call. By client 30 it's a moat.

The template is at `apollo/11-clients/README.md`. Five minutes per call. The most important five minutes you spend.

Links: [[sales]] · [[discovery]] · [[../PROJECT_BIBLE]]
