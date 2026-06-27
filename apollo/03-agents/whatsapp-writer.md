# WhatsApp Writer

> Opener. ≤ 40 words. One specific detail about the business. No links.

## Question answered
What is the shortest WhatsApp opener that earns a reply?

## Kind
LLM. Prompt: [[../04-prompts/whatsapp-writer]]. Gemini 2.5 Flash.

## Inputs
Same as Email Writer.

## Outputs
```json
{
  "message": "Hi Ahmed — saw ABC Dental does same-day consults. Quick idea on handling after-hours WhatsApp messages automatically without losing the personal touch. Worth 15 minutes this week?",
  "personalization_anchors": [
    { "type": "service", "text": "same-day consults" }
  ],
  "compliance_check": {
    "length_ok": true,
    "no_links": true,
    "no_emoji_spam": true
  }
}
```

## Schema rules
- Word count ≤ 40 (hard).
- No URLs in first message (WhatsApp anti-spam best practice).
- ≤ 1 emoji. None preferred.
- One personalization anchor minimum.
- No "Hi! How are you doing today?"-style preamble. Greeting + name + reason.

## Why so short
First WhatsApp messages over ~50 words read as spam and get reported. Short = high read-through. We earn the right to send the full pitch in message 2 after they reply.

## Failure modes
- Word cap violation → one retry → terminal.
- Link present → schema rejects.

## Cost ceiling
$0.003 per company.

## Emits
- `whatsapp.drafted`

## V1 acceptance
- 100% of goldens are ≤ 40 words and link-free.
- Operator approves ≥ 80% without edits.

Links: [[email-writer]] · [[proposal-writer]] · [[../04-prompts/whatsapp-writer]]
