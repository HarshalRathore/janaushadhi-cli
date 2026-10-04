# SPEC — v0.1

Aligned in a grilling session before the build. This captures the decisions and the
hard-won facts about the upstream API that the registry encodes.

## Scope

- **Every public endpoint** of the website API: 28 × `/api/v1/website/*` plus the
  state/district helper endpoints. Admin endpoints are out.
- Pharmacist directory **excluded** — verified not reachable with guest tokens
  (`/auth/getAllPharmacist` → 500, admin path → 401).
- Observation endpoints for archive (`--archived`) are included for tender, awarded
  tender, blacklist, debarred, revocation, report, gallery, image.

## Surface

Two layers:

1. **Uniform registry commands** — `janaushadhi <dataset> list|show [flags]` for every
   dataset; `<dataset>` defaults to `list` when no verb is given.
2. **Sugar** — `medicine search|list|show` (local filtering over the full catalog),
   `kendra list|show --state/--district/--pin/--store` (human names resolved to codes),
   `states`, `districts`.

Plus `datasets` (discoverability), `doctor` (health check), and
`api <path> [-d JSON] [-X METHOD]` (raw escape hatch, future-proofing).

## Output contract

- TTY default: table. Piped default: TSV. `--json` / `--csv` output never silently
  truncate (fetch everything unless `--limit N`).
- Table display caps at 100 rows with a `# showing N of M — use --all or --limit N`
  note on stderr.
- JSON schema: `{dataset, total, count, records:[…]}`; record keys mirror the API.
- CSV columns are curated per dataset (see registry).
- Exit codes: `0` ok, `1` not-found, `2` usage, `3` network/API.

## HTTP layer

- Spring Boot backend at `https://janaushadhi.gov.in:8443`; valid GlobalSign TLS cert;
  no Origin header required.
- Guest JWT (30 min) via `GET /auth/generateGuestToken`; cached 0600 in
  `~/.cache/janaushadhi/token.json`; refreshed 60 s before expiry and once on 401/403.
- One retry on 5xx / transport failure; UA `janaushadhi-cli/<version>`.
- **No data caching, ever** — every run fetches live.

## Upstream quirks encoded in the registry (do not forget)

- All list endpoints are POST with JSON `{pageIndex, pageSize, …}`; `pageSize` up to
  5000 is accepted. Medicines are fetched via 1000-row pages.
- Several endpoints return raw `500 server error` if `searchText` / `orderBy` /
  `columnName` keys are absent — the per-dataset `params` defaults carry them.
- `getAllContactByDepartment`: single call with `pageSize: 0` returns everything; no
  `totalElement` in the response.
- `getAllMediaEventByMonthAndYear`: requires `month` (full English name) **and** `year`.
- `getAllHospital`: 24k+ rows; `state`/`district` are free-text names; `hsplType`
  carries Public/GOI/….
- News `type` values: `WhatsNew` | `NewsLatter` (their spelling) — `all` fetches both.
- Kendra/distributor payloads use **stateCode** (`stateId`) and **districtCode**
  (`districtId`), not the master-table `id` values.
- Empty results sometimes come as envelope `responseCode: 404, "record not found"` —
  treated as zero rows.

## Identity

- Binary `janaushadhi`, crate `janaushadhi-cli`, repo
  https://github.com/HarshalRathore/janaushadhi-cli (public, MIT license).
- Upstream API is unofficial and may drift; `doctor` is the canary.
