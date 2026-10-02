# CONTEXT — janaushadhi-cli

Domain language for this repo. Definitions live here; do not redefine them elsewhere.

## Language

- **PMBJP** — *Pradhan Mantri Bhartiya Janaushadhi Pariyojana*, the government scheme
  that makes quality generic medicines affordable through dedicated outlets.
- **PMBI** — *Pharmaceuticals & Medical Devices Bureau of India*, the agency that runs
  PMBJP and publishes janaushadhi.gov.in.
- **Kendra (PMBJK)** — a Jan Aushadhi retail store. Store codes look like `PMBJK00455`.
- **Dataset** — one public API resource the CLI can fetch (medicine, kendra, tender,
  report, …). Declared once in `src/registry.rs`; used by `datasets`, `doctor`, and the
  dynamic `<dataset> list|show` commands.
- **Registry** — the static table of all datasets, their endpoints, columns and filters.
- **Sugar command** — a first-class command for a dataset that needs extra logic:
  `medicine` (local search over the whole catalog), `kendra` (name→code resolution),
  `states`, `districts`.
- **Archive mirror** — the `--archived` variant of a dataset, backed by the site's
  `getAllDelete*` endpoints.
- **Guest token** — short-lived (30 min) JWT issued by `GET /auth/generateGuestToken`.
  Required by every API call; cached at `~/.cache/janaushadhi/token.json`, refreshed
  automatically. This is the only thing the CLI ever caches.
- **Envelope** — the API's response wrapper `{responseBody, message, responseCode}`.
  `responseCode` can be non-200 inside an HTTP 200; `404 record not found` means an
  empty result (the CLI treats it as success-with-zero-rows).
- **State code vs state id** — the state master list carries both `id` and `stateCode`.
  Kendra/distributor filters want the **stateCode** (Delhi = 7, Rajasthan = 8).
  Districts analogously use `districtCode` (Central Delhi = 77).
- **Drug code** — the catalog's stable numeric medicine id (`drugCode`); `medicine
  show` takes it.
- **MRP** — maximum retail price in ₹ as published by PMBI; the full catalog is
  fetched as one dataset and filtered locally.
- **doctor** — the self-check command: verifies the token and fetches a sample from
  every registered endpoint; the early-warning system for upstream API drift.
