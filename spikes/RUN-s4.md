# S4 · Claude calls — RUN

**Goal:** make one minimal Claude Messages API call to a Haiku model and one to a
Sonnet model (raw HTTP — there is no official Anthropic Rust SDK), parse the
response, and print text + input/output tokens + computed cost. Confirms the §6
AI plumbing and cost accounting.

## 1. Put your API key in the repo-root `.env`

The key lives in the **gitignored** root `.env` (the spikes dir is one level
below it). `.env.example` is the tracked template:

```sh
# from the repo root (/Users/indrek/Development/personalCallAssistant)
cp .env.example .env
# then edit .env and set the real key:
#   ANTHROPIC_API_KEY=sk-ant-...
```

The spike loads `/Users/indrek/Development/personalCallAssistant/.env` via
`dotenvy::from_path`, so it works no matter which directory you run it from, and
the key persists across terminals (no manual `export`).

> The shipped app will use the macOS Keychain (§10); `.env` is the dev/spike path
> only. Never commit `.env`.

## 2. Run

```sh
cargo run --bin s4_claude
```

Expected output (token counts/cost will vary slightly):

```
--- Haiku 4.5 (claude-haiku-4-5) ---
reply:  spike ok
tokens: in=..., out=...
cost:   $0.0000xx

--- Sonnet 4.6 (claude-sonnet-4-6) ---
reply:  spike ok
tokens: in=..., out=...
cost:   $0.0000xx
```

## Models & pricing used (USD per million tokens)

| Model       | ID                  | input | output |
|-------------|---------------------|------:|-------:|
| Haiku 4.5   | `claude-haiku-4-5`  | $1.00 | $5.00  |
| Sonnet 4.6  | `claude-sonnet-4-6` | $3.00 | $15.00 |

Cost is computed as `in/1e6 * in_rate + out/1e6 * out_rate` from the `usage`
block the API returns.

## Troubleshooting

- **"ANTHROPIC_API_KEY not set"** → you didn't create `.env` or didn't set the
  key. See step 1.
- **"still looks like the placeholder"** → `.env` still has the `REPLACE_ME`
  template value.
- **HTTP 401** → invalid/revoked key.
- **HTTP 404 on a model** → model ID is wrong for your account; check
  `claude-haiku-4-5` / `claude-sonnet-4-6` against the current model list.
