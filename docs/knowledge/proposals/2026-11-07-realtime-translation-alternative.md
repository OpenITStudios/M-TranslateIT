---
id: document.knowledge.proposals.realtime-translation-alternative
class: DOCUMENT
domain: knowledge
role: ARCHITECTURE
authority: REFERENCE
lifecycle: DRAFT
---

# Dedicated OpenAI Realtime Translation — Separate Optional Provider

**Review date:** 2026-11-07. **Source/evidence:** OpenAI official model,
Realtime Translation guide and documentation, plus current TranslateIT `Local`
source. **State:** DESIGN ONLY; no network session, credentials, live model,
audio transmission, installed app test, provider implementation or authorization
for paid usage has occurred.

## Decision boundary

The user originally authorized exploration of included ChatGPT/Codex Voice
usage. That does not establish an OpenAI API credential, billing approval,
or permission to transmit meeting speech to a metered provider. The existing
Codex App Server is a coding-agent control surface. Its `clientManagedHandoffs`
flag cannot prevent all agent-turn initiation, and its realtime transcript
events do not demonstrate the source-to-final-translation identity required
to authorize output. Do not remove the existing Codex Meeting admission gate,
register an unsafe voice command, or claim a source-test-only adapter works.

The official **`gpt-realtime-translate`** model supplies a distinct route
specifically for translation:

- WebSocket URL: `wss://api.openai.com/v1/realtime/translations?model=gpt-realtime-translate`.
- Independent `OPENAI_API_KEY`/account authorization, usage limits and API
  billing; Codex/ChatGPT account login does not supply these.
- After opening the session, send `session.update` with
  `session.audio.output.language = "en"` for Indonesian outbound.
- Stream continuously using `session.input_audio_buffer.append` with Base64
  24-kHz mono PCM16, including silence; read
  `session.input_transcript.delta`, `session.output_transcript.delta`,
  `session.output_audio.delta` and `error` as separate evidence.
- On Stop, stop appending input, send `session.close` and wait for
  `session.closed` with a bounded drain deadline. Premature disconnect
  cannot be labeled clean completion.

The official model price at this review was **$0.034 per minute of audio**;
this is an informational reviewed price, not an account-specific quote,
entitlement, pricing guarantee, or authorization to incur charges.

Official references:
- https://developers.openai.com/api/docs/guides/realtime-translation
- https://developers.openai.com/api/docs/models/gpt-realtime-translate
- https://developers.openai.com/cookbook/examples/voice_solutions/realtime_translation_guide

## Translation pipeline design implications

`Local` remains the baseline and the default. A future separate API-powered
provider would be an explicit, user-consented option with visible cloud/billing
status, not a relabeling of `Codex` or a silent fallback:

```text
Local baseline:
physical mic -> native finalizer -> Faster Whisper -> MiLMMT
-> selected Built-in / approved My Voice (GPT-SoVITS)
-> existing VB-CABLE output -> real meeting receiver

Potential separately-authorized cloud alternative:
same native capture and Meeting owner -> 24-kHz audio sender
-> dedicated Realtime Translation session -> translated PCM
-> same native virtual output owner -> real meeting receiver
```

The cloud model returns **its own translated speech**. It does not establish
that the existing Built-in or approved My Voice will be used. The design
must either clearly expose its different voice output or independently prove
a final translated-text boundary before chaining GPT-SoVITS. Streaming
transcript deltas by themselves are not a final per-utterance TTS authority.
Do not promise voice consistency or reuse personal voice approval.

The existing Original Microphone, Mute, Start/Stop/Retry Stop, active device,
local Saved Transcripts policy, incoming caption lane and one-Meeting-owner
rules must be preserved. No simultaneous second microphone, no cloud audio
without explicit selection and permission, no silent Local fallback, no
implicit upload of archived conversations, no model credentials in Svelte,
HTML, diagnostics, logs or repository. A separate incoming EN-to-ID translation
session has its own authorization and billing; it is not automatically enabled.

## Required admission work before implementation

1. **Product authorization:** decide whether a separately metered API
   provider is desired despite the current included-usage-only Codex
   requirement; define where the user controls credentials and consents
   to transmitting live audio. No keys or financial acceptance are present
   in this repository.
2. **Native ownership:** design the smallest Windows-native transport under
   the current Meeting owner with bounded audio/frame queues, replay/Stop
   revocation, no second process that captures a physical microphone,
   and secure secret handling. Leave current Codex agent route sealed.
3. **Contract fixtures:** validate the exact event schema, error/finalization
   semantics, partial/done transcript meaning, PCM size/sample rate, queue
   overflow/backpressure, billing/connection refusals, and session.close
   ACK with fully synthetic offline fixtures. No recorded private speech.
4. **Source acceptance:** execute the existing locked Rust/frontend/worker
   verification on one clean pinned Local SHA. Avoid creating a second
   test harness or CI workflow while CI remains paused.
5. **Single integrated Windows session:** measure ID→EN accuracy,
   microphone capture, translated output reaching Zoom/Teams via VB-CABLE,
   latency P50/P95, mute/source switching, disconnection, Retry Stop and
   diagnostics with real target hardware. Separate cloud API runtime
   evidence from the unchanged Local baseline and from signed installer
   acceptance. Do not label paid API readiness or receiver success from
   WebSocket connection or audio-delta reception alone.

## Outcome and STOP condition

This document **does not activate a cloud provider**. It establishes a real,
supported translation-specific protocol alternative and the exact
product/credential/native conditions missing for adoption. The current Codex
App Server candidate stays diagnostic-only until its own upstream agent-free,
causally attributed audio-to-translation contract is proven. Neither route
may infer PASS from source presence, partial transcript deltas, an API key
format check, old CI results, or a user-selected Settings label.
