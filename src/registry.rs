//! Dataset registry: what the CLI can fetch, from which endpoint, and how to render it.
//!
//! Every public endpoint of the Jan Aushadhi website API is declared here once:
//! slug, endpoint path, archived variant, response list key, table columns and the
//! filters the endpoint accepts. Commands and `doctor` both iterate this registry.

use serde_json::{json, Map, Value};

use crate::api::{self, AppErr, Client};

#[derive(Clone)]
pub struct Col {
    pub key: &'static str,
    pub header: &'static str,
}

#[derive(Clone)]
pub struct Flags {
    /// payload key for `--search`
    pub search: Option<&'static str>,
    /// payload key for `--type`
    pub type_: Option<&'static str>,
    pub month: bool,
    pub year: bool,
    /// `--state`/`--district` resolve to stateId/districtId; `--pin`/`--store` allowed
    pub geo: bool,
    /// include `storeCode` in the payload (kendra)
    pub store_code: bool,
    /// `--state`/`--district` pass through as raw text (hospital)
    pub state_name: bool,
    /// record field for a client-side `--department` filter (contact)
    pub department: Option<&'static str>,
}

#[derive(Clone)]
pub struct Dataset {
    pub slug: &'static str,
    pub title: &'static str,
    pub path: &'static str,
    pub archived: Option<&'static str>,
    pub list_key: &'static str,
    pub columns: &'static [Col],
    pub flags: Flags,
    pub params: &'static [(&'static str, &'static str)],
    pub paginated: bool,
    pub show_id: bool,
    pub note: &'static str,
}

const NOFLAGS: Flags = Flags {
    search: None,
    type_: None,
    month: false,
    year: false,
    geo: false,
    store_code: false,
    state_name: false,
    department: None,
};

pub static DATASETS: &[Dataset] = &[
    Dataset {
        slug: "medicine",
        title: "Medicines — product portfolio (name, pack, MRP)",
        path: "/api/v1/website/getAllProductForWeb",
        archived: None,
        list_key: "newProductResponsesList",
        columns: &[
            Col { key: "drugCode", header: "Code" },
            Col { key: "genericName", header: "Generic Name" },
            Col { key: "groupName", header: "Group" },
            Col { key: "unitSize", header: "Pack" },
            Col { key: "mrp", header: "MRP (₹)" },
        ],
        flags: Flags { search: Some("searchText"), ..NOFLAGS },
        params: &[("orderBy", "asc"), ("columnName", "drug_code"), ("searchText", "")],
        paginated: true,
        show_id: true,
        note: "search/filters run locally over the full catalog",
    },
    Dataset {
        slug: "kendra",
        title: "Jan Aushadhi Kendras (store directory)",
        path: "/api/v1/website/getAllKendraByStateDistrict",
        archived: None,
        list_key: "addKendraResponseList",
        columns: &[
            Col { key: "storeCode", header: "Store Code" },
            Col { key: "contactPerson", header: "Contact" },
            Col { key: "contactNumber", header: "Phone" },
            Col { key: "kendraAddress", header: "Address" },
            Col { key: "districtName", header: "District" },
            Col { key: "stateName", header: "State" },
            Col { key: "pinCode", header: "PIN" },
        ],
        flags: Flags { geo: true, store_code: true, ..NOFLAGS },
        params: &[],
        paginated: true,
        show_id: true,
        note: "needs --state (name) or --pin or --store",
    },
    Dataset {
        slug: "distributor",
        title: "Distributors (state/district-wise)",
        path: "/api/v1/website/getAllDistributorByStateAndDistrict",
        archived: None,
        list_key: "distributorResponsePayloads",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "nameOfFirm", header: "Firm" },
            Col { key: "contactNumber", header: "Phone" },
            Col { key: "email", header: "Email" },
            Col { key: "distributorAddress", header: "Address" },
            Col { key: "districtName", header: "District" },
            Col { key: "stateName", header: "State" },
            Col { key: "pinCode", header: "PIN" },
        ],
        flags: Flags { geo: true, ..NOFLAGS },
        params: &[],
        paginated: true,
        show_id: true,
        note: "--state/--district take names",
    },
    Dataset {
        slug: "tender",
        title: "Tenders",
        path: "/api/v1/website/getAllTender",
        archived: Some("/api/v1/website/getAllDeleteTender"),
        list_key: "tenderResponsePayloadList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "tenderRefNo", header: "Ref No" },
            Col { key: "tenderTitle", header: "Title" },
            Col { key: "publishDate", header: "Published" },
            Col { key: "closingDate", header: "Closing" },
            Col { key: "status", header: "Status" },
        ],
        flags: Flags { search: Some("searchText"), ..NOFLAGS },
        params: &[("orderBy", "desc"), ("columnName", "publish_date"), ("searchText", "")],
        paginated: true,
        show_id: true,
        note: "file URLs in --json; --archived for old tenders",
    },
    Dataset {
        slug: "awarded-tender",
        title: "Awarded tenders",
        path: "/api/v1/website/getAllAwardedTender",
        archived: Some("/api/v1/website/getAllDeleteAwardedTender"),
        list_key: "awardedTenderResponseList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "department", header: "Department" },
            Col { key: "yearOfPurchase", header: "Year" },
            Col { key: "nitFile", header: "NIT File" },
            Col { key: "status", header: "Status" },
        ],
        flags: Flags { search: Some("searchText"), ..NOFLAGS },
        params: &[("orderBy", "desc"), ("columnName", "publish_date"), ("searchText", "")],
        paginated: true,
        show_id: true,
        note: "purchase list in --json",
    },
    Dataset {
        slug: "blacklist",
        title: "Blacklisted drugs",
        path: "/api/v1/website/getAllBlackList",
        archived: Some("/api/v1/website/getAllDeleteBlackList"),
        list_key: "blackListResponseList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "drugName", header: "Drug" },
            Col { key: "drugCode", header: "Code" },
            Col { key: "manufacturedBy", header: "Manufacturer" },
            Col { key: "fromDate", header: "From" },
            Col { key: "toDate", header: "To" },
            Col { key: "reason", header: "Reason" },
        ],
        flags: Flags { search: Some("searchText"), ..NOFLAGS },
        params: &[("orderBy", "desc"), ("columnName", "publish_date"), ("searchText", "")],
        paginated: true,
        show_id: true,
        note: "",
    },
    Dataset {
        slug: "debarred",
        title: "Debarred firms",
        path: "/api/v1/website/getAllDebarredList",
        archived: Some("/api/v1/website/getAllDeleteDebarredList"),
        list_key: "debarredResponseList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "drugName", header: "Drug" },
            Col { key: "drugCode", header: "Code" },
            Col { key: "manufacturedBy", header: "Manufacturer" },
            Col { key: "fromDate", header: "From" },
            Col { key: "toDate", header: "To" },
            Col { key: "reason", header: "Reason" },
        ],
        flags: Flags { search: Some("searchText"), ..NOFLAGS },
        params: &[("orderBy", "desc"), ("columnName", "publish_date"), ("searchText", "")],
        paginated: true,
        show_id: true,
        note: "",
    },
    Dataset {
        slug: "revocation",
        title: "Revocation orders",
        path: "/api/v1/website/getAllRevocationOrder",
        archived: Some("/api/v1/website/getAllDeleteRevocationOrder"),
        list_key: "revocationResponseList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "drugName", header: "Drug" },
            Col { key: "drugCode", header: "Code" },
            Col { key: "manufacturedBy", header: "Manufacturer" },
            Col { key: "blackListDate", header: "Blacklisted" },
            Col { key: "reason", header: "Reason" },
        ],
        flags: Flags { search: Some("searchText"), ..NOFLAGS },
        params: &[("orderBy", "desc"), ("columnName", "publish_date"), ("searchText", "")],
        paginated: true,
        show_id: true,
        note: "",
    },
    Dataset {
        slug: "report",
        title: "Annual/statutory reports",
        path: "/api/v1/website/getAllReportByType",
        archived: Some("/api/v1/website/getAllDeleteReportByType"),
        list_key: "annualAndFinancialResponseList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "title", header: "Title" },
            Col { key: "reportType", header: "Type" },
            Col { key: "docFile", header: "PDF" },
            Col { key: "createdDate", header: "Created" },
        ],
        flags: Flags { type_: Some("reportType"), ..NOFLAGS },
        params: &[("reportType", "Annual"), ("title", "")],
        paginated: true,
        show_id: true,
        note: "--type Annual (other values currently empty)",
    },
    Dataset {
        slug: "news",
        title: "What's New / Newsletters",
        path: "/api/v1/website/getAllByType",
        archived: None,
        list_key: "whatsNewAndNewsLatterList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "title", header: "Title" },
            Col { key: "type", header: "Type" },
            Col { key: "docFile", header: "Doc" },
            Col { key: "createdDate", header: "Created" },
        ],
        flags: Flags { type_: Some("type"), search: Some("searchByTitle"), ..NOFLAGS },
        params: &[("type", "WhatsNew"), ("searchByTitle", "")],
        paginated: true,
        show_id: true,
        note: "--type WhatsNew|NewsLatter|all",
    },
    Dataset {
        slug: "gallery",
        title: "Event gallery",
        path: "/api/v1/website/getAllEventGallery",
        archived: Some("/api/v1/website/getAllDeleteEventGallery"),
        list_key: "galleryResponseList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "eventTitle", header: "Event" },
            Col { key: "eventDate", header: "Date" },
            Col { key: "eventCategory", header: "Category" },
            Col { key: "eventName", header: "Name" },
        ],
        flags: Flags { search: Some("searchText"), ..NOFLAGS },
        params: &[("searchText", ""), ("orderBy", ""), ("columnName", "")],
        paginated: true,
        show_id: true,
        note: "image lists in --json",
    },
    Dataset {
        slug: "media-event",
        title: "Media events by month/year",
        path: "/api/v1/website/getAllMediaEventByMonthAndYear",
        archived: None,
        list_key: "mediaResponseList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "eventsTitle", header: "Event" },
            Col { key: "eventDate", header: "Date" },
            Col { key: "month", header: "Month" },
            Col { key: "year", header: "Year" },
        ],
        flags: Flags { month: true, year: true, ..NOFLAGS },
        params: &[],
        paginated: true,
        show_id: false,
        note: "needs --month <name> --year <yyyy>",
    },
    Dataset {
        slug: "image",
        title: "Images",
        path: "/api/v1/website/getAllImage",
        archived: Some("/api/v1/website/getAllDeleteImage"),
        list_key: "uploadImageLoopList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "title", header: "Title" },
            Col { key: "name", header: "Name" },
            Col { key: "category", header: "Category" },
            Col { key: "designation", header: "Designation" },
            Col { key: "imageUrl", header: "URL" },
        ],
        flags: Flags { search: Some("searchText"), ..NOFLAGS },
        params: &[("searchText", ""), ("orderBy", ""), ("columnName", "")],
        paginated: true,
        show_id: true,
        note: "",
    },
    Dataset {
        slug: "banner",
        title: "Website banners",
        path: "/api/v1/website/getAllBanner",
        archived: None,
        list_key: "bannerResponseLoopList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "bannerUrl", header: "Link" },
            Col { key: "bannerFile", header: "File" },
            Col { key: "status", header: "Status" },
            Col { key: "createdDate", header: "Created" },
        ],
        flags: NOFLAGS,
        params: &[("columnName", "id"), ("orderBy", "desc")],
        paginated: true,
        show_id: true,
        note: "",
    },
    Dataset {
        slug: "governing-council",
        title: "Governing Council members",
        path: "/api/v1/website/getAllGoverningCouncil",
        archived: None,
        list_key: "governanceConcilResponseList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "designation", header: "Designation" },
            Col { key: "gcMember", header: "Member" },
        ],
        flags: NOFLAGS,
        params: &[],
        paginated: true,
        show_id: true,
        note: "",
    },
    Dataset {
        slug: "executive-council",
        title: "Executive Council members",
        path: "/api/v1/website/getAllExecutiveCouncil",
        archived: None,
        list_key: "executiveResponseList",
        columns: &[
            Col { key: "id", header: "ID" },
            Col { key: "designation", header: "Designation" },
            Col { key: "ecMember", header: "Member" },
        ],
        flags: NOFLAGS,
        params: &[],
        paginated: true,
        show_id: true,
        note: "",
    },
    Dataset {
        slug: "management-team",
        title: "Management team",
        path: "/api/v1/website/getAllManagementTeam",
        archived: None,
        list_key: "managementResponseList",
        columns: &[
            Col { key: "name", header: "Name" },
            Col { key: "designation", header: "Designation" },
            Col { key: "eduQualification", header: "Qualification" },
            Col { key: "contactNo", header: "Phone" },
            Col { key: "mail", header: "Email" },
        ],
        flags: NOFLAGS,
        params: &[],
        paginated: true,
        show_id: true,
        note: "",
    },
    Dataset {
        slug: "hospital",
        title: "PM Jan Aushadhi hospitals (co-located kendras)",
        path: "/api/v1/website/getAllHospital",
        archived: None,
        list_key: "hospitalResponseList",
        columns: &[
            Col { key: "hsplid", header: "Hospital ID" },
            Col { key: "hsplName", header: "Hospital" },
            Col { key: "hsplType", header: "Type" },
            Col { key: "state", header: "State" },
            Col { key: "district", header: "District" },
            Col { key: "address", header: "Address" },
            Col { key: "pincode", header: "PIN" },
        ],
        flags: Flags { type_: Some("hsplType"), state_name: true, ..NOFLAGS },
        params: &[("hsplType", ""), ("state", ""), ("district", "")],
        paginated: true,
        show_id: true,
        note: "24k+ records; --state/--district free text; --type Public|GOI|...",
    },
    Dataset {
        slug: "contact",
        title: "Organisation contacts (by department)",
        path: "/api/v1/website/getAllContactByDepartment",
        archived: None,
        list_key: "contactResponseList",
        columns: &[
            Col { key: "name", header: "Name" },
            Col { key: "designation", header: "Designation" },
            Col { key: "department", header: "Department" },
            Col { key: "contactNo", header: "Phone" },
            Col { key: "intercomNo", header: "Intercom" },
            Col { key: "emailId", header: "Email" },
            Col { key: "location", header: "Location" },
        ],
        flags: Flags { department: Some("department"), ..NOFLAGS },
        params: &[("searchText", ""), ("orderBy", ""), ("columnName", "")],
        paginated: false,
        show_id: false,
        note: "--department filters locally",
    },
];

/// Resolve a user-supplied slug (accepts common plural forms).
pub fn find(slug: &str) -> Option<&'static Dataset> {
    let slug = match slug {
        "medicines" => "medicine",
        "kendras" => "kendra",
        "distributors" => "distributor",
        "tenders" => "tender",
        "reports" => "report",
        "galleries" => "gallery",
        "banners" => "banner",
        "images" => "image",
        "hospitals" => "hospital",
        "contacts" => "contact",
        other => other,
    };
    DATASETS.iter().find(|d| d.slug == slug)
}

pub fn filters_desc(ds: &Dataset) -> String {
    let f = &ds.flags;
    let mut v: Vec<&str> = Vec::new();
    if f.search.is_some() {
        v.push("--search");
    }
    if f.type_.is_some() {
        v.push("--type");
    }
    if f.month {
        v.push("--month");
    }
    if f.year {
        v.push("--year");
    }
    if f.geo {
        v.push("--state/--district by name");
        v.push("--pin");
        if f.store_code {
            v.push("--store");
        }
    }
    if f.state_name {
        v.push("--state/--district (text)");
    }
    if f.department.is_some() {
        v.push("--department");
    }
    v.join(", ")
}

/// Fetch records for a dataset, looping pages until done (or `cap` reached).
/// Returns `(records, total)` where `total` is the server's totalElement when known.
pub fn fetch_records(
    client: &mut Client,
    ds: &Dataset,
    base: &Map<String, Value>,
    cap: Option<usize>,
) -> Result<(Vec<Value>, Option<u64>), AppErr> {
    let mut out: Vec<Value> = Vec::new();
    let mut total: Option<u64> = None;

    if !ds.paginated {
        let mut payload = base.clone();
        payload.insert("pageIndex".to_string(), json!(0));
        payload.insert("pageSize".to_string(), json!(0));
        let v = client.post_json(ds.path, &Value::Object(payload))?;
        if let Some(body) = api::unwrap_response(v)? {
            total = body.get("totalElement").and_then(|t| t.as_u64());
            if let Some(list) = body.get(ds.list_key).and_then(|l| l.as_array()) {
                out.extend(list.iter().cloned());
            } else if let Some(arr) = body.as_array() {
                out.extend(arr.iter().cloned());
            }
        }
        if let Some(c) = cap {
            out.truncate(c);
        }
        return Ok((out, total));
    }

    let page_size: usize = 1000;
    let mut page: usize = 0;
    loop {
        let remaining = cap.map(|c| c.saturating_sub(out.len()));
        if remaining == Some(0) {
            break;
        }
        let mut payload = base.clone();
        payload.insert("pageIndex".to_string(), json!(page));
        let want = remaining.unwrap_or(page_size).min(page_size).max(1);
        payload.insert("pageSize".to_string(), json!(want));
        let v = client.post_json(ds.path, &Value::Object(payload))?;
        let body = match api::unwrap_response(v)? {
            None => break,
            Some(b) => b,
        };
        if total.is_none() {
            total = body.get("totalElement").and_then(|t| t.as_u64());
        }
        let list = body
            .get(ds.list_key)
            .and_then(|l| l.as_array())
            .cloned()
            .unwrap_or_default();
        if list.is_empty() {
            break;
        }
        out.extend(list);
        if let Some(c) = cap {
            if out.len() >= c {
                out.truncate(c);
                break;
            }
        }
        if let Some(t) = total {
            if out.len() as u64 >= t {
                break;
            }
        }
        if body.get("isLastPage").and_then(|b| b.as_bool()).unwrap_or(false) {
            break;
        }
        page += 1;
        if page > 200 {
            break;
        }
    }
    Ok((out, total))
}

/// Minimal payload for health checks (doctor): defaults + a sample state for geo datasets.
pub fn sample_payload(ds: &Dataset, state_code: u64) -> Map<String, Value> {
    let mut m = Map::new();
    for (k, v) in ds.params {
        m.insert((*k).to_string(), parse_scalar(v));
    }
    if ds.flags.geo {
        m.insert("stateId".to_string(), json!(state_code));
        m.insert("districtId".to_string(), json!(0));
        m.insert("pinCode".to_string(), json!(0));
        if ds.flags.store_code {
            m.insert("storeCode".to_string(), json!(""));
        }
    }
    if ds.flags.month {
        m.insert("month".to_string(), json!("January"));
    }
    if ds.flags.year {
        m.insert("year".to_string(), json!(2000));
    }
    m
}

pub fn parse_scalar(s: &str) -> Value {
    if let Ok(n) = s.parse::<i64>() {
        return json!(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return json!(f);
    }
    match s {
        "true" => json!(true),
        "false" => json!(false),
        _ => json!(s),
    }
}

/// Local filter for the medicine catalog: case-insensitive, whitespace-split tokens
/// are ANDed across name / group / drug code.
pub fn filter_medicines(
    records: &[Value],
    search: Option<&str>,
    group: Option<&str>,
    code: Option<u64>,
) -> Vec<Value> {
    let group_q = group.map(|g| g.to_lowercase());
    let tokens: Vec<String> = search
        .map(|s| s.to_lowercase().split_whitespace().map(String::from).collect())
        .unwrap_or_default();
    records
        .iter()
        .filter(|r| {
            if let Some(c) = code {
                if r.get("drugCode").and_then(|v| v.as_u64()) != Some(c) {
                    return false;
                }
            }
            if let Some(gq) = &group_q {
                let gname = r
                    .get("groupName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                if !gname.contains(gq.as_str()) {
                    return false;
                }
            }
            if !tokens.is_empty() {
                let name = r.get("genericName").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                let grp = r.get("groupName").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                let dcode = r
                    .get("drugCode")
                    .and_then(|v| v.as_u64())
                    .map(|c| c.to_string())
                    .unwrap_or_default();
                let hay = format!("{name} {grp} {dcode}");
                if !tokens.iter().all(|t| hay.contains(t.as_str())) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

/// Find one record by a human-supplied id across common ID fields.
pub fn find_by_id(records: &[Value], id: &str) -> Option<Value> {
    let q = id.trim().to_lowercase();
    const ID_KEYS: [&str; 6] = ["id", "drugCode", "storeCode", "productId", "hsplid", "hsplno"];
    records
        .iter()
        .find(|r| {
            ID_KEYS.iter().any(|k| match r.get(*k) {
                Some(Value::Number(n)) => n.to_string().to_lowercase() == q,
                Some(Value::String(s)) => s.to_lowercase() == q,
                _ => false,
            })
        })
        .cloned()
}

pub fn resolve_state(client: &mut Client, q: &str) -> Result<u64, AppErr> {
    let states = api::get_states(client)?;
    match_state(&states, q).map_err(api::usage)
}

/// Match a state by name (exact then unique substring) or by code/id.
/// Returns the *stateCode*, which is what the kendra/distributor APIs expect.
pub fn match_state(states: &[Value], q: &str) -> Result<u64, String> {
    let qt = q.trim();
    if let Ok(n) = qt.parse::<u64>() {
        for s in states {
            if s.get("stateCode").and_then(|v| v.as_u64()) == Some(n) {
                return Ok(n);
            }
        }
        for s in states {
            if s.get("id").and_then(|v| v.as_u64()) == Some(n) {
                if let Some(c) = s.get("stateCode").and_then(|v| v.as_u64()) {
                    return Ok(c);
                }
            }
        }
        return Err(format!("no state with code {n} — run `janaushadhi states`"));
    }
    let name_of =
        |s: &Value| s.get("stateNameInEnglish").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if let Some(s) = states.iter().find(|s| name_of(s).eq_ignore_ascii_case(qt)) {
        return s
            .get("stateCode")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| format!("state '{}' has no code", name_of(s)));
    }
    let ql = qt.to_lowercase();
    let subs: Vec<&Value> = states
        .iter()
        .filter(|s| name_of(s).to_lowercase().contains(&ql))
        .collect();
    match subs.len() {
        0 => Err(format!("no state matching '{qt}' — run `janaushadhi states`")),
        1 => subs[0]
            .get("stateCode")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| "state has no code".to_string()),
        _ => Err(format!(
            "'{qt}' is ambiguous: {}",
            subs.iter().map(|s| name_of(s)).collect::<Vec<_>>().join(", ")
        )),
    }
}

pub fn resolve_district(client: &mut Client, state_code: u64, q: &str) -> Result<u64, AppErr> {
    let districts = api::get_districts(client, state_code)?;
    match_district(&districts, q).map_err(api::usage)
}

/// Match a district by name or code; returns the *districtCode* used in payloads.
pub fn match_district(districts: &[Value], q: &str) -> Result<u64, String> {
    let qt = q.trim();
    if let Ok(n) = qt.parse::<u64>() {
        for d in districts {
            if d.get("districtCode").and_then(|v| v.as_u64()) == Some(n) {
                return Ok(n);
            }
        }
        for d in districts {
            if d.get("id").and_then(|v| v.as_u64()) == Some(n) {
                if let Some(c) = d.get("districtCode").and_then(|v| v.as_u64()) {
                    return Ok(c);
                }
            }
        }
        return Err(format!("no district with code {n} in that state"));
    }
    let name_of = |d: &Value| {
        d.get("districtNameInEnglish").and_then(|v| v.as_str()).unwrap_or("").to_string()
    };
    if let Some(d) = districts.iter().find(|d| name_of(d).eq_ignore_ascii_case(qt)) {
        return d
            .get("districtCode")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| format!("district '{}' has no code", name_of(d)));
    }
    let ql = qt.to_lowercase();
    let subs: Vec<&Value> = districts
        .iter()
        .filter(|d| name_of(d).to_lowercase().contains(&ql))
        .collect();
    match subs.len() {
        0 => Err(format!("no district matching '{qt}' in that state")),
        1 => subs[0]
            .get("districtCode")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| "district has no code".to_string()),
        _ => Err(format!(
            "'{qt}' is ambiguous: {}",
            subs.iter().map(|d| name_of(d)).collect::<Vec<_>>().join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medicines_filter_tokens() {
        let recs = vec![
            json!({"drugCode": 1, "genericName": "Aceclofenac 100mg and Paracetamol 325mg Tablets",
                   "groupName": "Analgesic/Antipyretic/Anti-Inflammatory"}),
            json!({"drugCode": 70, "genericName": "Metformin Hydrochloride Tablets IP 500 mg",
                   "groupName": "Anti-Diabetic"}),
        ];
        let f = filter_medicines(&recs, Some("metformin 500"), None, None);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0]["drugCode"], json!(70));
        assert_eq!(filter_medicines(&recs, None, Some("analgesic"), None).len(), 1);
        assert_eq!(filter_medicines(&recs, None, None, Some(1)).len(), 1);
        assert_eq!(filter_medicines(&recs, Some("zzz"), None, None).len(), 0);
    }

    #[test]
    fn state_matching() {
        let states = vec![
            json!({"id": 8, "stateCode": 7, "stateNameInEnglish": "Delhi"}),
            json!({"id": 22, "stateCode": 8, "stateNameInEnglish": "Rajasthan"}),
        ];
        assert_eq!(match_state(&states, "delhi").unwrap(), 7);
        assert_eq!(match_state(&states, "7").unwrap(), 7);
        assert_eq!(match_state(&states, "8").unwrap(), 8);
        assert_eq!(match_state(&states, "22").unwrap(), 8);
        assert!(match_state(&states, "atlantis").is_err());
    }

    #[test]
    fn scalar_parsing() {
        assert_eq!(parse_scalar("desc"), json!("desc"));
        assert_eq!(parse_scalar("42"), json!(42));
        assert_eq!(parse_scalar("true"), json!(true));
    }

    #[test]
    fn find_by_id_matches_strings_and_numbers() {
        let recs = vec![json!({"id": 5, "storeCode": "PMBJK00455"})];
        assert!(find_by_id(&recs, "5").is_some());
        assert!(find_by_id(&recs, "pmbjk00455").is_some());
        assert!(find_by_id(&recs, "nope").is_none());
    }
}
