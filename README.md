# janaushadhi

Unofficial command-line tool for the **Jan Aushadhi (PMBJP)** public REST API —
search the medicine catalog with MRPs, locate kendras, pull tenders, reports, news,
gallery data and more, straight from `janaushadhi.gov.in`.

> **Disclaimer:** This is an unofficial tool. It is not affiliated with PMBI or the
> Government of India. All data is fetched live from the public website API at
> `https://janaushadhi.gov.in:8443` and displayed as-published. Nothing here is
> medical advice; prices and availability change — verify at your kendra.

## Install

Requires a Rust toolchain (edition 2024, recent stable).

```bash
cargo install --path .        # installs `janaushadhi` into ~/.cargo/bin
# or
cargo build --release         # binary at target/release/janaushadhi
```

## Quick start

```bash
janaushadhi medicine search metformin              # find medicines by name
janaushadhi medicine show 70                       # one medicine by drug code
janaushadhi medicine list --group "anti-diabetic"  # filter by therapeutic group

janaushadhi kendra list --state delhi --pin 110001 # stores by PIN code
janaushadhi kendra show PMBJK00455                 # one store by store code
janaushadhi states                                 # state names + codes
janaushadhi districts --state delhi                # districts of a state

janaushadhi tender list --limit 10                 # live tenders
janaushadhi tender list --archived --all           # old tenders
janaushadhi report list                            # annual reports (PDF links)
janaushadhi news list --type all                   # What's New + Newsletters
janaushadhi hospital list --state Delhi            # co-located hospital kendras
janaushadhi contact list --department Media        # organisation contacts

janaushadhi datasets                               # every dataset + endpoint + filters
janaushadhi doctor                                 # token + endpoint health check
janaushadhi api /api/v1/website/getAllTender -d '{"pageIndex":0,"pageSize":5}'
```

## Commands

| command | what it does |
|---|---|
| `medicine list\|search\|show` | product portfolio (2.4k medicines) with MRP; search/filters run locally over one live fetch |
| `kendra list\|show` | store directory; `--state` (name or code), `--district`, `--pin`, `--store` |
| `states` / `districts --state X` | reference data used by the `--state`/`--district` filters |
| `<dataset> list [flags]` | **any** registered dataset: tender, awarded-tender, blacklist, debarred, revocation, report, news, gallery, media-event, image, banner, governing-council, executive-council, management-team, hospital, contact, distributor |
| `<dataset> show <id>` | one record by id, where available |
| `datasets` | list every dataset with endpoint and supported filters |
| `doctor` | fetch a sample from every endpoint; catches API drift |
| `api <path> [-d JSON] [-X METHOD]` | raw call against any public endpoint |

### Global flags

`--json`, `--csv`, `--table`, `--no-color`, `--all`, `--limit N`, `-v/--verbose`

### Dataset flags (per dataset — see `janaushadhi datasets`)

`--search Q`, `--type T`, `--month NAME`, `--year YYYY`, `--state`, `--district`,
`--pin`, `--store`, `--department`, `--title`, `--column C`, `--order asc|desc`,
`--filter key=value` (raw payload override), `--archived`

## Output contract

- On a TTY the default is a **table**; when piped the default is **TSV** (header + rows).
- `--json` and `--csv` are for machines and are **never silently truncated** (they fetch
  everything unless you pass `--limit`).
- Tables show the first 100 rows with a `# showing N of M` note on **stderr**
  (`--all` or `--limit N` to change).
- Data goes to stdout; notes/diagnostics go to stderr.

### Exit codes

| code | meaning |
|---|---|
| 0 | success |
| 1 | requested record not found |
| 2 | usage error (bad flags, unknown dataset) |
| 3 | network or API failure |

## Sessions & caching

- Every command fetches **live data** — nothing is stored locally.
- The only thing cached is the short-lived guest JWT, at
  `~/.cache/janaushadhi/token.json` (0600), auto-refreshed before expiry and on 401/403.
  You never have to think about auth.
- The CLI sends a descriptive `janaushadhi-cli/<version>` User-Agent and retries once
  on transient 5xx/network errors.

## Troubleshooting

- Something failing? Run `janaushadhi doctor` — it exercises every endpoint and shows
  which part of the upstream API changed. Add `-v` anywhere for HTTP diagnostics.
- The upstream API is unofficial and can change without notice (columns, filters,
  payload quirks). The registry lives in `src/registry.rs` — one line per dataset.

## License

MIT. See LICENSE. Data © the respective government publishers; this tool only reads
their public API.
