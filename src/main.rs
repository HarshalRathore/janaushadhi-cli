//! janaushadhi — unofficial CLI for the Jan Aushadhi (PMBJP) public REST API.
//!
//! Data is fetched live from janaushadhi.gov.in:8443 on every run. Only the guest
//! token is cached (in ~/.cache/janaushadhi). Not affiliated with PMBI.

mod api;
mod output;
mod registry;

use std::ffi::OsString;

use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{json, Map, Value};

use api::{ApiError, AppErr, Client};
use output::Mode;
use registry::{Col, Dataset, DATASETS};

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December",
];

#[derive(Parser)]
#[command(
    name = "janaushadhi",
    version,
    about = "Unofficial CLI for the Jan Aushadhi (PMBJP) public REST API.\nData is fetched live from janaushadhi.gov.in — not affiliated with PMBI.",
    after_help = "Datasets:\n  janaushadhi datasets            list every dataset with endpoint + filters\n  janaushadhi <dataset> list      fetch a dataset (e.g. tender, report, news, gallery,\n                                  image, banner, hospital, contact, distributor, ...)\n  janaushadhi medicine search Q   search the medicine catalog by text\n  janaushadhi kendra list         find stores: --state --district --pin --store\n  janaushadhi states / districts  reference data for filters\n  janaushadhi doctor              token + endpoint health check\n  janaushadhi api <path> -d JSON  raw call against any public endpoint\n\nOutput: table on a TTY, TSV when piped; --json / --csv for machines; # notes go to stderr."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,

    /// Raw JSON output
    #[arg(long, global = true)]
    json: bool,
    /// CSV output
    #[arg(long, global = true)]
    csv: bool,
    /// Force table output
    #[arg(long, global = true)]
    table: bool,
    /// Disable colors (tables are uncolored; flag accepted for compatibility)
    #[arg(long = "no-color", global = true)]
    no_color: bool,
    /// Fetch all records (tables otherwise show the first 100)
    #[arg(long, global = true)]
    all: bool,
    /// Limit to N records
    #[arg(long, global = true, value_name = "N")]
    limit: Option<usize>,
    /// Verbose diagnostics on stderr
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Medicines: search / list the product portfolio with MRP
    Medicine {
        #[command(subcommand)]
        sub: MedCmd,
    },
    /// Kendras: locate Jan Aushadhi stores
    Kendra {
        #[command(subcommand)]
        sub: KendraCmd,
    },
    /// List states (the codes used by --state filters)
    States,
    /// List districts of a state
    Districts {
        /// State name or code
        #[arg(long, value_name = "STATE")]
        state: String,
    },
    /// List every dataset, its endpoint and filters
    Datasets,
    /// Check guest-token handling and endpoint health
    Doctor,
    /// Raw API call, e.g. `janaushadhi api /api/v1/website/getAllTender -d '{}'`
    Api {
        /// API path starting with /api or /auth
        path: String,
        /// JSON request body (implies POST unless -X is given)
        #[arg(short = 'd', long, value_name = "JSON")]
        data: Option<String>,
        /// HTTP method: GET or POST
        #[arg(short = 'X', long, value_name = "METHOD")]
        method: Option<String>,
    },
    #[command(external_subcommand)]
    External(Vec<OsString>),
}

#[derive(Subcommand)]
enum MedCmd {
    /// List medicines (local filter over the whole live catalog)
    List {
        /// Substring search across name / group / code
        #[arg(long, short)]
        search: Option<String>,
        /// Filter by therapeutic group
        #[arg(long, short)]
        group: Option<String>,
        /// Exact drug code
        #[arg(long, short)]
        code: Option<u64>,
    },
    /// Search medicines by text (space-separated words are ANDed)
    Search {
        #[arg(required = true, num_args = 1.., value_name = "QUERY")]
        query: Vec<String>,
    },
    /// Show one medicine by drug code
    Show {
        /// Drug code, e.g. 70
        code: u64,
    },
}

#[derive(Subcommand)]
enum KendraCmd {
    /// List kendras (needs --state, --pin or --store)
    List {
        /// State name or code
        #[arg(long)]
        state: Option<String>,
        /// District name or code (requires --state)
        #[arg(long)]
        district: Option<String>,
        /// Filter by PIN code
        #[arg(long)]
        pin: Option<i64>,
        /// Look up one store code without a state
        #[arg(long)]
        store: Option<String>,
    },
    /// Show one kendra by store code, e.g. PMBJK00455
    Show {
        /// Store code
        store: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(e.exit_code());
        }
    }
}

fn run(cli: &Cli) -> Result<(), AppErr> {
    let mode = output::mode_from(cli.json, cli.csv, cli.table);
    let _ = cli.no_color; // tables are uncolored by default
    match &cli.cmd {
        None => {
            let mut cmd = Cli::command();
            let _ = cmd.print_help();
            println!();
            Ok(())
        }
        Some(Cmd::Medicine { sub }) => {
            let mut client = Client::new(cli.verbose);
            cmd_medicine(&mut client, cli, sub, mode)
        }
        Some(Cmd::Kendra { sub }) => {
            let mut client = Client::new(cli.verbose);
            cmd_kendra(&mut client, cli, sub, mode)
        }
        Some(Cmd::States) => {
            let mut client = Client::new(cli.verbose);
            cmd_states(&mut client, mode)
        }
        Some(Cmd::Districts { state }) => {
            let mut client = Client::new(cli.verbose);
            cmd_districts(&mut client, mode, state)
        }
        Some(Cmd::Datasets) => cmd_datasets(),
        Some(Cmd::Doctor) => {
            let mut client = Client::new(cli.verbose);
            cmd_doctor(&mut client)
        }
        Some(Cmd::Api { path, data, method }) => {
            let mut client = Client::new(cli.verbose);
            cmd_api(&mut client, path, data.as_deref(), method.as_deref())
        }
        Some(Cmd::External(args)) => cmd_external(cli, args, mode),
    }
}

/// How many records to fetch: everything with --all; N with --limit; otherwise
/// tables cap at 100 while --json/--csv fetch all (never silently truncated).
fn fetch_cap(cli: &Cli, mode: Mode) -> Option<usize> {
    if cli.all {
        None
    } else if let Some(n) = cli.limit {
        Some(n)
    } else if mode == Mode::Table {
        Some(100)
    } else {
        None
    }
}

fn present(
    mode: Mode,
    cols: &[Col],
    title: &str,
    records: &[Value],
    note_total: Option<u64>,
) -> Result<(), AppErr> {
    match mode {
        Mode::Json => output::render_json(title, records, note_total),
        Mode::Csv => output::render_csv(cols, records)?,
        Mode::Tsv => output::render_tsv(cols, records),
        Mode::Table => output::render_table(cols, records),
    }
    if let Some(t) = note_total {
        if t > records.len() as u64 {
            eprintln!("# showing {} of {} — use --all or --limit N", records.len(), t);
        }
    }
    Ok(())
}

fn present_record(mode: Mode, ds: &Dataset, rec: &Value) -> Result<(), AppErr> {
    match mode {
        Mode::Json => {
            output::print_value(rec);
            Ok(())
        }
        Mode::Csv => output::render_csv(ds.columns, std::slice::from_ref(rec)),
        _ => {
            output::print_kv(rec);
            Ok(())
        }
    }
}

fn cmd_medicine(client: &mut Client, cli: &Cli, sub: &MedCmd, mode: Mode) -> Result<(), AppErr> {
    let ds = registry::find("medicine").expect("medicine dataset is registered");
    let base = registry::sample_payload(ds, 0);
    let (records, _total) = registry::fetch_records(client, ds, &base, None)?;
    match sub {
        MedCmd::List { search, group, code } => {
            let filtered = registry::filter_medicines(&records, search.as_deref(), group.as_deref(), *code);
            if filtered.is_empty() {
                return Err(AppErr::NotFound("no medicines match the filter".to_string()));
            }
            let cap = fetch_cap(cli, mode);
            let shown: Vec<Value> = match cap {
                Some(c) => filtered.iter().take(c).cloned().collect(),
                None => filtered.clone(),
            };
            present(mode, ds.columns, "medicine", &shown, Some(filtered.len() as u64))
        }
        MedCmd::Search { query } => {
            let q = query.join(" ");
            let filtered = registry::filter_medicines(&records, Some(&q), None, None);
            if filtered.is_empty() {
                return Err(AppErr::NotFound(format!("no medicines match '{q}'")));
            }
            let cap = fetch_cap(cli, mode);
            let shown: Vec<Value> = match cap {
                Some(c) => filtered.iter().take(c).cloned().collect(),
                None => filtered.clone(),
            };
            present(mode, ds.columns, "medicine", &shown, Some(filtered.len() as u64))
        }
        MedCmd::Show { code } => {
            let rec = records
                .iter()
                .find(|r| r.get("drugCode").and_then(|v| v.as_u64()) == Some(*code))
                .cloned();
            match rec {
                Some(r) => present_record(mode, ds, &r),
                None => Err(AppErr::NotFound(format!("no medicine with drug code {code}"))),
            }
        }
    }
}

fn cmd_kendra(client: &mut Client, cli: &Cli, sub: &KendraCmd, mode: Mode) -> Result<(), AppErr> {
    let ds = registry::find("kendra").expect("kendra dataset is registered");
    match sub {
        KendraCmd::List { state, district, pin, store } => {
            let state_code = match state {
                Some(s) => Some(registry::resolve_state(client, s)?),
                None => None,
            };
            let district_code = match district {
                Some(d) => {
                    let sc = state_code
                        .ok_or_else(|| api::usage("--district needs --state"))?;
                    Some(registry::resolve_district(client, sc, d)?)
                }
                None => None,
            };
            if state_code.is_none() && pin.is_none() && store.is_none() {
                return Err(api::usage(
                    "give --state (or --pin / --store): the kendra directory needs a state",
                ));
            }
            let mut payload = Map::new();
            payload.insert("stateId".to_string(), json!(state_code.unwrap_or(0)));
            payload.insert("districtId".to_string(), json!(district_code.unwrap_or(0)));
            payload.insert("pinCode".to_string(), json!(pin.unwrap_or(0)));
            payload.insert("storeCode".to_string(), json!(store.clone().unwrap_or_default()));
            let cap = fetch_cap(cli, mode);
            let (records, total) = registry::fetch_records(client, ds, &payload, cap)?;
            if records.is_empty() && mode == Mode::Table {
                eprintln!("# no kendras found for those filters");
                return Ok(());
            }
            present(mode, ds.columns, ds.slug, &records, total)
        }
        KendraCmd::Show { store } => {
            let mut payload = Map::new();
            payload.insert("stateId".to_string(), json!(0));
            payload.insert("districtId".to_string(), json!(0));
            payload.insert("pinCode".to_string(), json!(0));
            payload.insert("storeCode".to_string(), json!(store));
            let (records, _) = registry::fetch_records(client, ds, &payload, Some(2))?;
            let rec = records
                .into_iter()
                .find(|r| {
                    r.get("storeCode")
                        .and_then(|v| v.as_str())
                        .map(|s| s.eq_ignore_ascii_case(store))
                        .unwrap_or(false)
                })
                .ok_or_else(|| AppErr::NotFound(format!("no kendra with store code '{store}'")))?;
            present_record(mode, ds, &rec)
        }
    }
}

fn cmd_states(client: &mut Client, mode: Mode) -> Result<(), AppErr> {
    let states = api::get_states(client)?;
    let cols = [
        Col { key: "stateCode", header: "Code" },
        Col { key: "stateNameInEnglish", header: "State" },
        Col { key: "stateNameInLocal", header: "Local" },
        Col { key: "stateOrUt", header: "T/U" },
    ];
    present(mode, &cols, "states", &states, Some(states.len() as u64))
}

fn cmd_districts(client: &mut Client, mode: Mode, state: &str) -> Result<(), AppErr> {
    let state_code = registry::resolve_state(client, state)?;
    let districts = api::get_districts(client, state_code)?;
    let cols = [
        Col { key: "districtCode", header: "Code" },
        Col { key: "districtNameInEnglish", header: "District" },
        Col { key: "stateNameInEnglish", header: "State" },
    ];
    present(mode, &cols, "districts", &districts, Some(districts.len() as u64))
}

fn cmd_datasets() -> Result<(), AppErr> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for ds in DATASETS {
        let mut endpoint = ds.path.to_string();
        if let Some(a) = ds.archived {
            endpoint.push_str("  (+archived: ");
            endpoint.push_str(a);
            endpoint.push(')');
        }
        rows.push(vec![
            ds.slug.to_string(),
            ds.title.to_string(),
            endpoint,
            registry::filters_desc(ds),
            ds.note.to_string(),
        ]);
    }
    output::rows_table(&["dataset", "what", "endpoint", "filters", "notes"], rows);
    println!();
    println!("Use:  janaushadhi <dataset> list [flags]     e.g. janaushadhi tender list --search supply");
    println!("      janaushadhi <dataset> show <id>        for datasets that support it");
    println!("      janaushadhi api <path> -d '<json>'     raw call against any public endpoint");
    println!("Output flags: --json | --csv | --table | --all | --limit N    (-v for diagnostics)");
    Ok(())
}

fn cmd_doctor(client: &mut Client) -> Result<(), AppErr> {
    let exp = client.token_expiry()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mins = exp.saturating_sub(now) / 60;
    println!("janaushadhi doctor");
    println!("base URL : {}", api::BASE_URL);
    println!("token    : ok (expires in ~{mins} min)\n");

    let states = api::get_states(client).unwrap_or_default();
    let first_state = states
        .iter()
        .find_map(|s| s.get("stateCode").and_then(|v| v.as_u64()))
        .unwrap_or(7);

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut failures = 0usize;
    for ds in DATASETS {
        let payload = registry::sample_payload(ds, first_state);
        let (status, detail) = match registry::fetch_records(client, ds, &payload, Some(1)) {
            Ok((recs, total)) => {
                if recs.is_empty() {
                    ("ok".to_string(), "(reachable, no records)".to_string())
                } else {
                    (
                        "ok".to_string(),
                        format!("total {}", total.map(|t| t.to_string()).unwrap_or_else(|| "?".to_string())),
                    )
                }
            }
            Err(e) => {
                failures += 1;
                ("FAIL".to_string(), e.to_string())
            }
        };
        rows.push(vec![ds.slug.to_string(), status, detail]);
    }
    output::rows_table(&["dataset", "status", "detail"], rows);
    if failures > 0 {
        return Err(AppErr::Api(ApiError::Network(format!("{failures} endpoint(s) failing"))));
    }
    Ok(())
}

fn cmd_api(client: &mut Client, path: &str, data: Option<&str>, method: Option<&str>) -> Result<(), AppErr> {
    if !(path.starts_with("/api") || path.starts_with("/auth")) {
        return Err(api::usage("path must start with /api or /auth"));
    }
    let method = match method {
        Some(m) => m.to_uppercase(),
        None => {
            if data.is_some() {
                "POST".to_string()
            } else {
                "GET".to_string()
            }
        }
    };
    let payload: Option<Value> = match data {
        Some(s) => Some(
            serde_json::from_str(s).map_err(|e| api::usage(format!("invalid JSON in -d: {e}")))?,
        ),
        None => None,
    };
    let v = match method.as_str() {
        "POST" => client.post_json(path, &payload.unwrap_or_else(|| json!({})))?,
        "GET" => client.get_json(path)?,
        other => return Err(api::usage(format!("unsupported method {other} (use GET or POST)"))),
    };
    output::print_value(&v);
    if let Some(code) = v.get("responseCode").and_then(|c| c.as_u64()) {
        if code != 200 {
            let msg = v.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();
            eprintln!("note: server responseCode {code}: {msg}");
            return Err(AppErr::Api(ApiError::Server { code, message: msg }));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Dynamic dataset commands: `janaushadhi <dataset> list|show [flags]`
// ---------------------------------------------------------------------------

enum DSub {
    List,
    Show(String),
}

struct DArgs {
    sub: DSub,
    search: Option<String>,
    type_: Option<String>,
    title: Option<String>,
    department: Option<String>,
    state: Option<String>,
    district: Option<String>,
    pin: Option<i64>,
    store: Option<String>,
    month: Option<String>,
    year: Option<i64>,
    column: Option<String>,
    order: Option<String>,
    filters: Vec<(String, Value)>,
    archived: bool,
    all: bool,
    limit: Option<usize>,
    json: bool,
    csv: bool,
    table: bool,
    verbose: bool,
}

fn take_val(
    toks: &[String],
    i: &mut usize,
    name: &str,
    inline: &Option<String>,
) -> Result<String, AppErr> {
    if let Some(v) = inline {
        return Ok(v.clone());
    }
    *i += 1;
    toks.get(*i)
        .cloned()
        .ok_or_else(|| api::usage(format!("missing value for --{name}")))
}

fn parse_filter_value(v: &str) -> Value {
    if let Ok(n) = v.parse::<i64>() {
        return json!(n);
    }
    if let Ok(f) = v.parse::<f64>() {
        return json!(f);
    }
    match v {
        "true" => json!(true),
        "false" => json!(false),
        _ => json!(v),
    }
}

fn parse_dargs(toks: &[String]) -> Result<DArgs, AppErr> {
    let mut a = DArgs {
        sub: DSub::List,
        search: None,
        type_: None,
        title: None,
        department: None,
        state: None,
        district: None,
        pin: None,
        store: None,
        month: None,
        year: None,
        column: None,
        order: None,
        filters: Vec::new(),
        archived: false,
        all: false,
        limit: None,
        json: false,
        csv: false,
        table: false,
        verbose: false,
    };
    let mut i = 0;
    while i < toks.len() {
        let tok = toks[i].clone();
        if tok == "list" {
            a.sub = DSub::List;
            i += 1;
            continue;
        }
        if tok == "show" {
            let nxt = toks
                .get(i + 1)
                .filter(|t| !t.starts_with('-'))
                .ok_or_else(|| api::usage("`show` needs an id, e.g. `show 42`"))?;
            a.sub = DSub::Show(nxt.clone());
            i += 2;
            continue;
        }
        if let Some(rest) = tok.strip_prefix("--") {
            let (name, inline) = match rest.split_once('=') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (rest.to_string(), None),
            };
            match name.as_str() {
                "search" => a.search = Some(take_val(toks, &mut i, &name, &inline)?),
                "type" => a.type_ = Some(take_val(toks, &mut i, &name, &inline)?),
                "title" => a.title = Some(take_val(toks, &mut i, &name, &inline)?),
                "department" => a.department = Some(take_val(toks, &mut i, &name, &inline)?),
                "state" => a.state = Some(take_val(toks, &mut i, &name, &inline)?),
                "district" => a.district = Some(take_val(toks, &mut i, &name, &inline)?),
                "pin" => {
                    let v = take_val(toks, &mut i, &name, &inline)?;
                    a.pin = Some(v.parse().map_err(|_| api::usage("--pin must be a number"))?);
                }
                "store" => a.store = Some(take_val(toks, &mut i, &name, &inline)?),
                "month" => a.month = Some(take_val(toks, &mut i, &name, &inline)?),
                "year" => {
                    let v = take_val(toks, &mut i, &name, &inline)?;
                    a.year = Some(v.parse().map_err(|_| api::usage("--year must be a number"))?);
                }
                "column" => a.column = Some(take_val(toks, &mut i, &name, &inline)?),
                "order" => {
                    let v = take_val(toks, &mut i, &name, &inline)?;
                    if v != "asc" && v != "desc" {
                        return Err(api::usage("--order must be asc or desc"));
                    }
                    a.order = Some(v);
                }
                "filter" => {
                    let kv = take_val(toks, &mut i, &name, &inline)?;
                    let (k, v) = kv
                        .split_once('=')
                        .ok_or_else(|| api::usage("--filter needs key=value"))?;
                    a.filters.push((k.to_string(), parse_filter_value(v)));
                }
                "limit" => {
                    let v = take_val(toks, &mut i, &name, &inline)?;
                    a.limit = Some(v.parse().map_err(|_| api::usage("--limit must be a number"))?);
                }
                "all" => a.all = true,
                "json" => a.json = true,
                "csv" => a.csv = true,
                "table" => a.table = true,
                "no-color" => {}
                "archived" => a.archived = true,
                "verbose" => a.verbose = true,
                other => return Err(api::usage(format!("unknown flag --{other}"))),
            }
            i += 1;
            continue;
        }
        if tok == "-v" {
            a.verbose = true;
            i += 1;
            continue;
        }
        return Err(api::usage(format!("unexpected argument '{tok}'")));
    }
    Ok(a)
}

fn build_payload(client: &mut Client, ds: &Dataset, inv: &DArgs) -> Result<Map<String, Value>, AppErr> {
    let mut m: Map<String, Value> = Map::new();
    for (k, v) in ds.params {
        m.insert((*k).to_string(), registry::parse_scalar(v));
    }
    if inv.title.is_some() && !ds.params.iter().any(|(k, _)| *k == "title") {
        return Err(api::usage("this dataset has no --title"));
    }
    if ds.flags.geo {
        let mut state_code = 0u64;
        let mut district_code = 0u64;
        if let Some(s) = &inv.state {
            state_code = registry::resolve_state(client, s)?;
        }
        if let Some(d) = &inv.district {
            if inv.state.is_none() {
                return Err(api::usage("--district needs --state"));
            }
            district_code = registry::resolve_district(client, state_code, d)?;
        }
        m.insert("stateId".to_string(), json!(state_code));
        m.insert("districtId".to_string(), json!(district_code));
        m.insert("pinCode".to_string(), json!(inv.pin.unwrap_or(0)));
        if ds.flags.store_code {
            m.insert("storeCode".to_string(), json!(inv.store.clone().unwrap_or_default()));
        } else if inv.store.is_some() {
            return Err(api::usage("this dataset has no --store"));
        }
    } else {
        if inv.state.is_some() || inv.district.is_some() {
            if ds.flags.state_name {
                if let Some(s) = &inv.state {
                    m.insert("state".to_string(), json!(s));
                }
                if let Some(d) = &inv.district {
                    m.insert("district".to_string(), json!(d));
                }
            } else {
                return Err(api::usage("this dataset has no --state/--district filter"));
            }
        }
        if inv.pin.is_some() {
            return Err(api::usage("this dataset has no --pin"));
        }
        if inv.store.is_some() {
            return Err(api::usage("this dataset has no --store"));
        }
    }
    if let Some(s) = &inv.search {
        match ds.flags.search {
            Some(k) => {
                m.insert(k.to_string(), json!(s));
            }
            None => return Err(api::usage("this dataset has no --search")),
        }
    }
    if let Some(t) = &inv.type_ {
        match ds.flags.type_ {
            Some(k) => {
                m.insert(k.to_string(), json!(t));
            }
            None => return Err(api::usage("this dataset has no --type")),
        }
    }
    if let Some(t) = &inv.title {
        m.insert("title".to_string(), json!(t));
    }
    if let Some(mo) = &inv.month {
        if !ds.flags.month {
            return Err(api::usage("this dataset has no --month"));
        }
        let canon = MONTHS
            .iter()
            .find(|mm| mm.eq_ignore_ascii_case(mo.trim()))
            .ok_or_else(|| api::usage(format!("--month must be a month name (got '{mo}')")))?;
        m.insert("month".to_string(), json!(canon));
    }
    if let Some(y) = inv.year {
        if !ds.flags.year {
            return Err(api::usage("this dataset has no --year"));
        }
        m.insert("year".to_string(), json!(y));
    }
    if inv.department.is_some() && ds.flags.department.is_none() {
        return Err(api::usage("this dataset has no --department"));
    }
    if let Some(c) = &inv.column {
        m.insert("columnName".to_string(), json!(c));
    }
    if let Some(o) = &inv.order {
        m.insert("orderBy".to_string(), json!(o));
    }
    for (k, v) in &inv.filters {
        m.insert(k.clone(), v.clone());
    }
    if ds.flags.month && ds.flags.year && (inv.month.is_none() || inv.year.is_none()) {
        return Err(api::usage("this dataset needs both --month <name> and --year <yyyy>"));
    }
    Ok(m)
}

fn cmd_external(cli: &Cli, args: &[OsString], base_mode: Mode) -> Result<(), AppErr> {
    let toks: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let slug = toks.first().ok_or_else(|| api::usage("missing dataset name"))?;
    let ds = registry::find(slug)
        .ok_or_else(|| api::usage(format!("unknown dataset '{slug}' — run `janaushadhi datasets`")))?;
    let inv = parse_dargs(&toks[1..])?;

    let mut client = Client::new(cli.verbose || inv.verbose);
    let mode = if inv.json {
        Mode::Json
    } else if inv.csv {
        Mode::Csv
    } else if inv.table {
        Mode::Table
    } else {
        base_mode
    };
    let all = cli.all || inv.all;
    let limit = inv.limit.or(cli.limit);
    let cap = if all {
        None
    } else if let Some(n) = limit {
        Some(n)
    } else if mode == Mode::Table {
        Some(100)
    } else {
        None
    };

    let ds_use = if inv.archived {
        let p = ds
            .archived
            .ok_or_else(|| api::usage(format!("dataset '{slug}' has no --archived variant")))?;
        Dataset { path: p, ..ds.clone() }
    } else {
        ds.clone()
    };

    match &inv.sub {
        DSub::Show(id) => {
            if !ds.show_id {
                return Err(api::usage(format!(
                    "dataset '{slug}' has no `show` — use `janaushadhi {slug} list`"
                )));
            }
            let payload = build_payload(&mut client, &ds_use, &inv)?;
            let (records, _) = registry::fetch_records(&mut client, &ds_use, &payload, Some(5000))?;
            let rec = registry::find_by_id(&records, id)
                .ok_or_else(|| AppErr::NotFound(format!("no {slug} record with id '{id}'")))?;
            present_record(mode, &ds_use, &rec)
        }
        DSub::List => {
            let payload = build_payload(&mut client, &ds_use, &inv)?;
            let news_all = slug == "news"
                && inv
                    .type_
                    .as_deref()
                    .map(|t| t.eq_ignore_ascii_case("all"))
                    .unwrap_or(false);
            let (mut records, mut total) = if news_all {
                let mut p1 = payload.clone();
                p1.insert("type".to_string(), json!("WhatsNew"));
                let mut p2 = payload.clone();
                p2.insert("type".to_string(), json!("NewsLatter"));
                let (r1, t1) = registry::fetch_records(&mut client, &ds_use, &p1, cap)?;
                let (r2, t2) = registry::fetch_records(&mut client, &ds_use, &p2, cap)?;
                let mut all_recs = r1;
                all_recs.extend(r2);
                let tot = match (t1, t2) {
                    (Some(a), Some(b)) => Some(a + b),
                    _ => None,
                };
                (all_recs, tot)
            } else {
                registry::fetch_records(&mut client, &ds_use, &payload, cap)?
            };
            if let (Some(field), Some(q)) = (ds.flags.department, inv.department.as_deref()) {
                let ql = q.to_lowercase();
                records.retain(|r| {
                    r.get(field)
                        .and_then(|v| v.as_str())
                        .map(|x| x.to_lowercase().contains(&ql))
                        .unwrap_or(false)
                });
                if records.is_empty() {
                    return Err(AppErr::NotFound(format!("no records with {field} matching '{q}'")));
                }
                total = Some(records.len() as u64);
            }
            if records.is_empty() && mode == Mode::Table {
                eprintln!("# no records for `{slug}` with those filters");
                return Ok(());
            }
            present(mode, ds_use.columns, ds_use.slug, &records, total)
        }
    }
}
