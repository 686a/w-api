use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use encoding_rs::{EUC_KR, UTF_8};
use mongodb::bson::doc;
use reqwest::{
    Client, StatusCode, Url,
    header::{CONTENT_TYPE, COOKIE, HeaderValue},
};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::{
    api::remote_client,
    errors::{ErrorResponse, internal_error},
    state::GlobalState,
};

const DEFAULT_SM: &str = "3";

#[derive(Clone, Copy)]
struct InfoPageDefinition {
    key: &'static str,
    section: &'static str,
    title: &'static str,
    path: &'static str,
    sm: &'static str,
    auto_follow_form: Option<&'static str>,
}

const INFO_PAGES: &[InfoPageDefinition] = &[
    InfoPageDefinition {
        key: "timetable-room",
        section: "시간표관리",
        title: "강의실별시간표조회",
        path: "/SWupis/V005/Service/Stud/TimeTable/timeTableByRoom.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "timetable-range",
        section: "시간표관리",
        title: "영역별시간표",
        path: "/SWupis/V005/Service/Stud/TimeTable/timeTableByRange.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "timetable-dept",
        section: "시간표관리",
        title: "학과별시간표",
        path: "/SWupis/V005/Service/Stud/TimeTable/timeTableByDept.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "curriculum-range",
        section: "시간표관리",
        title: "교육과정조회(영역별)",
        path: "/SWupis/V005/Service/Stud/Course/listByRange.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "curriculum-dept",
        section: "시간표관리",
        title: "교육과정조회(학과별)",
        path: "/SWupis/V005/Service/Stud/Course/listByDept.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "curriculum-micro-degree",
        section: "시간표관리",
        title: "교육과정조회(마이크로디그리전공)",
        path: "/SWupis/V005/Service/Stud/Print/MdInfo.jsp",
        sm: "2",
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "same-lessons",
        section: "시간표관리",
        title: "동일과목조회",
        path: "/SWupis/V005/Service/Stud/Course/sameLessonList.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "professor-timetable",
        section: "시간표관리",
        title: "교수시간표조회",
        path: "/SWupis/V005/Service/Stud/LecPlan/searchProfScheInfo.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "lecture-plan",
        section: "수업관리",
        title: "강의계획서조회",
        path: "/SWupis/V005/Service/Stud/LecPlan/searchLecPlanInfo.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "course-registrations",
        section: "수업관리",
        title: "수강신청조회",
        path: "/SWupis/V005/Service/Stud/Sugang/Request/requestList.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "lecture-evaluation",
        section: "수업관리",
        title: "수업평가실시",
        path: "/SWupis/V005/Service/Stud/Estimate/e072/getEstimateLecture.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "lecture-evaluation-result",
        section: "수업관리",
        title: "수업평가결과보기",
        path: "/SWupis/V005/Service/Emplo/Estimate/list.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "midterm-lecture-evaluation",
        section: "수업관리",
        title: "중간수업평가실시",
        path: "/SWupis/V005/Service/Stud/Estimate/e072/getEstimateMidLecture.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "midterm-lecture-evaluation-result",
        section: "수업관리",
        title: "중간수업평가결과보기",
        path: "/SWupis/V005/Service/Emplo/Estimate/list.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "class-rest-lecture",
        section: "수업관리",
        title: "휴보강조회",
        path: "/SWupis/V005/Service/Stud/TimeTable/restLecture.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "attendance",
        section: "전자출결관리",
        title: "출결조회",
        path: "/SWupis/V005/Service/Stud/Absent/StudAttend/studAttendList.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "attendance-rest-lecture",
        section: "전자출결관리",
        title: "휴보강조회",
        path: "/SWupis/V005/Service/Stud/Absent/StudAttend/studCanSupList.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "approved-absence",
        section: "전자출결관리",
        title: "공결조회",
        path: "/SWupis/V005/Service/Stud/Absent/applyAbsenceList.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "season-course",
        section: "계절학기관리",
        title: "시간표조회",
        path: "/SWupis/V005/Service/Stud/Season/Course/course.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "season-registration-list",
        section: "계절학기관리",
        title: "수강신청조회",
        path: "/SWupis/V005/Service/Stud/Season/Request/seasonRequest.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "season-bill",
        section: "계절학기관리",
        title: "등록고지서출력",
        path: "/SWupis/V005/Service/Stud/Season/Print/print.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "season-score",
        section: "계절학기관리",
        title: "계절학기성적조회",
        path: "/SWupis/V005/Service/Stud/Score/scoreSeason.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "score-table",
        section: "성적관리",
        title: "성적단표내역조회",
        path: "/SWupis/V005/Service/Stud/Score/scoreTable.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "score-drop-lessons",
        section: "성적관리",
        title: "성적포기내역조회",
        path: "/SWupis/V005/Service/Stud/Score/dropLessonInfo.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "pre-college-score",
        section: "성적관리",
        title: "예비대학성적조회",
        path: "/SWupis/V005/Service/Stud/Score/scorePre.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "completed-courses",
        section: "성적관리",
        title: "이수과목확인리스트",
        path: "/SWupis/V005/Service/Stud/Print/studComplete.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "liberal-arts-credits",
        section: "성적관리",
        title: "교양영역이수학점조회",
        path: "/SWupis/V005/Service/Stud/Print/studLbrlart.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: Some("na_print"),
    },
    InfoPageDefinition {
        key: "micro-degree-completion",
        section: "성적관리",
        title: "마이크로디그리이수내역",
        path: "/SWupis/V005/Service/Stud/Print/studMdComp.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
    InfoPageDefinition {
        key: "all-scores",
        section: "성적관리",
        title: "전체성적조회",
        path: "/SWupis/V005/Service/Stud/Score/scoreAll.jsp",
        sm: DEFAULT_SM,
        auto_follow_form: None,
    },
];

#[derive(Deserialize)]
pub struct InfoPagesQuery {
    token: Option<String>,
}

#[derive(Deserialize)]
pub struct InfoPageQuery {
    token: Option<String>,
    #[serde(flatten)]
    params: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct InfoPagesResult {
    error: bool,
    error_message: Option<String>,
    pages: Vec<InfoPageSummary>,
}

#[derive(Serialize)]
pub struct InfoPageSummary {
    key: &'static str,
    section: &'static str,
    title: &'static str,
    path: &'static str,
}

#[derive(Serialize)]
pub struct InfoPageResult {
    error: bool,
    error_message: Option<String>,
    page: InfoPageSummary,
    request: InfoPageRequest,
    alerts: Vec<String>,
    text: String,
    forms: Vec<ParsedForm>,
    tables: Vec<ParsedTable>,
}

#[derive(Serialize)]
pub struct InfoPageRequest {
    path: String,
    params: HashMap<String, String>,
    auto_followed_form: Option<String>,
}

#[derive(Serialize)]
pub struct ParsedForm {
    index: usize,
    name: Option<String>,
    method: Option<String>,
    action: Option<String>,
    controls: Vec<FormControl>,
}

#[derive(Serialize)]
pub struct FormControl {
    tag: String,
    input_type: Option<String>,
    name: Option<String>,
    id: Option<String>,
    value: Option<String>,
    text: String,
    options: Vec<SelectOption>,
}

#[derive(Serialize)]
pub struct SelectOption {
    value: String,
    text: String,
    selected: bool,
}

#[derive(Serialize)]
pub struct ParsedTable {
    index: usize,
    rows: Vec<ParsedTableRow>,
}

#[derive(Serialize)]
pub struct ParsedTableRow {
    header: bool,
    cells: Vec<String>,
}

pub async fn pages(
    State(state): State<GlobalState>,
    headers: HeaderMap,
    Query(query): Query<InfoPagesQuery>,
) -> Result<Json<InfoPagesResult>, (StatusCode, Json<ErrorResponse>)> {
    let token = get_token(&headers, query.token.as_deref())?;
    get_session_cookie(&state, token).await?;

    Ok(Json(InfoPagesResult {
        error: false,
        error_message: None,
        pages: INFO_PAGES.iter().map(page_summary).collect(),
    }))
}

pub async fn page(
    State(state): State<GlobalState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Query(query): Query<InfoPageQuery>,
) -> Result<Json<InfoPageResult>, (StatusCode, Json<ErrorResponse>)> {
    let token = get_token(&headers, query.token.as_deref())?;
    let session_cookie = get_session_cookie(&state, token).await?;
    let page = info_page(&key)?;
    let client = remote_client().map_err(internal_error)?;
    let params = sanitize_params(query.params);
    let mut html = fetch_info_page_html(
        &client,
        &state.remote_api_domain,
        page,
        &session_cookie,
        &params,
    )
    .await?;
    let mut fetched_path = page.path.to_string();
    let mut auto_followed_form = None;

    if let Some(form_name) = page.auto_follow_form {
        if let Some(followed) = follow_form(
            &client,
            &state.remote_api_domain,
            page,
            &session_cookie,
            &html,
            form_name,
        )
        .await?
        {
            html = followed.html;
            fetched_path = followed.path;
            auto_followed_form = Some(form_name.to_string());
        }
    }

    let parsed = parse_info_page(&html)?;
    if page_unavailable(&parsed) {
        return Err(public_error(
            StatusCode::CONFLICT,
            parsed
                .alerts
                .first()
                .map(String::as_str)
                .unwrap_or("Page is unavailable"),
        ));
    }

    Ok(Json(InfoPageResult {
        error: false,
        error_message: None,
        page: page_summary(page),
        request: InfoPageRequest {
            path: fetched_path,
            params,
            auto_followed_form,
        },
        alerts: parsed.alerts,
        text: parsed.text,
        forms: parsed.forms,
        tables: parsed.tables,
    }))
}

struct ParsedInfoPage {
    alerts: Vec<String>,
    text: String,
    forms: Vec<ParsedForm>,
    tables: Vec<ParsedTable>,
}

struct FollowedForm {
    path: String,
    html: String,
}

async fn get_session_cookie(
    state: &GlobalState,
    token: &str,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let sessions = state
        .db_client
        .collection::<mongodb::bson::Document>("sessions");
    let session = sessions
        .find_one(doc! { "token": token })
        .await
        .map_err(internal_error)?;

    let Some(session) = session else {
        return Err(public_error(StatusCode::UNAUTHORIZED, "Failed to login"));
    };

    session
        .get_str("login_session_cookie")
        .map(str::to_string)
        .map_err(|_| public_error(StatusCode::UNAUTHORIZED, "Failed to login"))
}

async fn fetch_info_page_html(
    client: &Client,
    remote_api_domain: &str,
    page: &InfoPageDefinition,
    session_cookie: &str,
    params: &HashMap<String, String>,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let mut url = info_url(remote_api_domain, page.path)?;
    append_page_query(&mut url, page, params);

    let response = client
        .get(url)
        .header(COOKIE, session_cookie)
        .send()
        .await
        .map_err(remote_error)?;

    if !response.status().is_success() {
        return Err(public_error(
            StatusCode::BAD_GATEWAY,
            "Failed to fetch remote page",
        ));
    }

    decode_response(response).await
}

async fn follow_form(
    client: &Client,
    remote_api_domain: &str,
    page: &InfoPageDefinition,
    session_cookie: &str,
    html: &str,
    form_name: &str,
) -> Result<Option<FollowedForm>, (StatusCode, Json<ErrorResponse>)> {
    let (target_url, form_values) = {
        let document = Html::parse_document(html);
        let form_selector = selector("form")?;
        let Some(form) = document.select(&form_selector).find(|form| {
            form.value()
                .attr("name")
                .is_some_and(|name| name == form_name)
        }) else {
            return Ok(None);
        };

        let action = form
            .value()
            .attr("action")
            .filter(|action| !action.is_empty())
            .unwrap_or(page.path);
        let base_url = info_url(remote_api_domain, page.path)?;
        let target_url = base_url
            .join(action)
            .map_err(|_| public_error(StatusCode::BAD_GATEWAY, "Invalid remote form action"))?;
        let form_values = form_values(&form)?;

        (target_url, form_values)
    };

    let response = client
        .post(target_url.clone())
        .header(COOKIE, session_cookie)
        .form(&form_values)
        .send()
        .await
        .map_err(remote_error)?;

    if !response.status().is_success() {
        return Err(public_error(
            StatusCode::BAD_GATEWAY,
            "Failed to fetch remote page",
        ));
    }

    let path = target_url.path().to_string()
        + target_url
            .query()
            .map(|query| format!("?{query}"))
            .unwrap_or_default()
            .as_str();
    let html = decode_response(response).await?;

    Ok(Some(FollowedForm { path, html }))
}

async fn decode_response(
    response: reqwest::Response,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let content_type = response.headers().get(CONTENT_TYPE).cloned();
    let bytes = response.bytes().await.map_err(remote_error)?;

    Ok(decode_html(&bytes, content_type.as_ref()))
}

fn parse_info_page(html: &str) -> Result<ParsedInfoPage, (StatusCode, Json<ErrorResponse>)> {
    let document = Html::parse_document(html);
    let text_document = Html::parse_document(&strip_non_content_blocks(html));
    let body_selector = selector("body")?;
    let text = text_document
        .select(&body_selector)
        .next()
        .map(|body| clean_multiline_text(&element_text(&body)))
        .unwrap_or_default();

    Ok(ParsedInfoPage {
        alerts: extract_alert_messages(html),
        text,
        forms: parse_forms(&document)?,
        tables: parse_tables(&document)?,
    })
}

fn parse_forms(document: &Html) -> Result<Vec<ParsedForm>, (StatusCode, Json<ErrorResponse>)> {
    let form_selector = selector("form")?;
    let control_selector = selector("input, select, textarea, button")?;

    Ok(document
        .select(&form_selector)
        .enumerate()
        .map(|(index, form)| ParsedForm {
            index,
            name: attr(&form, "name"),
            method: attr(&form, "method").map(|value| value.to_ascii_uppercase()),
            action: attr(&form, "action"),
            controls: form
                .select(&control_selector)
                .map(|control| form_control(&control))
                .collect(),
        })
        .collect())
}

fn form_control(control: &ElementRef<'_>) -> FormControl {
    FormControl {
        tag: control.value().name().to_string(),
        input_type: attr(control, "type"),
        name: attr(control, "name"),
        id: attr(control, "id"),
        value: control_value(control),
        text: clean_text(&element_text(control)),
        options: select_options(control),
    }
}

fn parse_tables(document: &Html) -> Result<Vec<ParsedTable>, (StatusCode, Json<ErrorResponse>)> {
    let table_selector = selector("table")?;
    let row_selector = selector("tr")?;
    let cell_selector = selector("th, td")?;

    Ok(document
        .select(&table_selector)
        .enumerate()
        .filter_map(|(index, table)| {
            let rows = table
                .select(&row_selector)
                .filter_map(|row| {
                    let cells = row
                        .select(&cell_selector)
                        .map(|cell| clean_text(&element_text(&cell)))
                        .filter(|text| !text.is_empty())
                        .collect::<Vec<_>>();
                    if cells.is_empty() {
                        None
                    } else {
                        Some(ParsedTableRow {
                            header: row
                                .select(&cell_selector)
                                .any(|cell| cell.value().name() == "th"),
                            cells,
                        })
                    }
                })
                .collect::<Vec<_>>();

            (!rows.is_empty()).then_some(ParsedTable { index, rows })
        })
        .collect())
}

fn form_values(
    form: &ElementRef<'_>,
) -> Result<Vec<(String, String)>, (StatusCode, Json<ErrorResponse>)> {
    let control_selector = selector("input, select, textarea")?;

    Ok(form
        .select(&control_selector)
        .filter_map(|control| {
            let name = attr(&control, "name")?;
            if name.is_empty() {
                return None;
            }

            Some((name, control_value(&control).unwrap_or_default()))
        })
        .collect())
}

fn select_options(control: &ElementRef<'_>) -> Vec<SelectOption> {
    if control.value().name() != "select" {
        return Vec::new();
    }

    let Ok(option_selector) = Selector::parse("option") else {
        return Vec::new();
    };

    control
        .select(&option_selector)
        .map(|option| SelectOption {
            value: attr(&option, "value").unwrap_or_default(),
            text: clean_text(&element_text(&option)),
            selected: option.value().attr("selected").is_some(),
        })
        .collect()
}

fn control_value(control: &ElementRef<'_>) -> Option<String> {
    if control.value().name() == "select" {
        let option_selector = Selector::parse("option").ok()?;
        let selected = control
            .select(&option_selector)
            .find(|option| option.value().attr("selected").is_some())
            .or_else(|| control.select(&option_selector).next());
        return selected
            .and_then(|option| attr(&option, "value"))
            .or_else(|| selected.map(|option| clean_text(&element_text(&option))));
    }

    attr(control, "value")
}

fn attr(element: &ElementRef<'_>, name: &str) -> Option<String> {
    element
        .value()
        .attr(name)
        .map(clean_text)
        .filter(|value| !value.is_empty())
}

fn page_unavailable(parsed: &ParsedInfoPage) -> bool {
    !parsed.alerts.is_empty()
        && parsed.forms.is_empty()
        && parsed.tables.is_empty()
        && parsed.text.is_empty()
}

fn info_page(key: &str) -> Result<&'static InfoPageDefinition, (StatusCode, Json<ErrorResponse>)> {
    INFO_PAGES
        .iter()
        .find(|page| page.key == key)
        .ok_or_else(|| public_error(StatusCode::NOT_FOUND, "Unknown info service page"))
}

fn page_summary(page: &InfoPageDefinition) -> InfoPageSummary {
    InfoPageSummary {
        key: page.key,
        section: page.section,
        title: page.title,
        path: page.path,
    }
}

fn sanitize_params(params: HashMap<String, String>) -> HashMap<String, String> {
    params
        .into_iter()
        .filter(|(key, _)| key != "token")
        .map(|(key, value)| (clean_text(&key), clean_text(&value)))
        .filter(|(key, _)| !key.is_empty())
        .collect()
}

fn append_page_query(url: &mut Url, page: &InfoPageDefinition, params: &HashMap<String, String>) {
    let mut query = url.query_pairs_mut();
    if !page.sm.is_empty() {
        query.append_pair("sm", page.sm);
    }
    for (key, value) in params {
        query.append_pair(key, value);
    }
}

fn info_url(remote_api_domain: &str, path: &str) -> Result<Url, (StatusCode, Json<ErrorResponse>)> {
    Url::parse(&format!("https://intra.{remote_api_domain}{path}"))
        .map_err(|_| public_error(StatusCode::INTERNAL_SERVER_ERROR, "Invalid remote URL"))
}

fn decode_html(bytes: &[u8], content_type: Option<&HeaderValue>) -> String {
    let content_type = content_type
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let charset_source = format!("{content_type}\n{prefix}").to_ascii_lowercase();
    let encoding = if charset_source.contains("utf-8") || charset_source.contains("utf8") {
        UTF_8
    } else {
        EUC_KR
    };
    let (text, _, _) = encoding.decode(bytes);

    text.into_owned()
}

fn extract_alert_messages(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut cursor = 0;
    let mut messages = Vec::new();
    let mut seen = HashSet::new();

    while let Some(found) = lower[cursor..].find("alert") {
        let alert_index = cursor + found;
        let Some(open_offset) = lower[alert_index..].find('(') else {
            break;
        };
        let mut index = alert_index + open_offset + 1;
        while html[index..].starts_with(char::is_whitespace) {
            index += html[index..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
        }

        let Some(quote) = html[index..].chars().next() else {
            break;
        };
        if quote != '"' && quote != '\'' {
            cursor = index.saturating_add(quote.len_utf8());
            continue;
        }

        index += quote.len_utf8();
        let mut escaped = false;
        let mut message = String::new();
        let mut end_index = index;
        for character in html[index..].chars() {
            end_index += character.len_utf8();
            if escaped {
                message.push(match character {
                    'n' | 'r' => ' ',
                    't' => ' ',
                    other => other,
                });
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == quote {
                break;
            }
            message.push(character);
        }

        let message = clean_text(&message);
        if !message.is_empty() && seen.insert(message.clone()) {
            messages.push(message);
        }
        cursor = end_index;
    }

    messages
}

fn strip_non_content_blocks(html: &str) -> String {
    let without_scripts = strip_tag_blocks(html, "script");
    let without_styles = strip_tag_blocks(&without_scripts, "style");
    strip_tag_blocks(&without_styles, "noscript")
}

fn strip_tag_blocks(html: &str, tag: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut cursor = 0;
    let mut output = String::new();

    while let Some(open_offset) = lower[cursor..].find(&open) {
        let open_index = cursor + open_offset;
        output.push_str(&html[cursor..open_index]);
        if let Some(close_offset) = lower[open_index..].find(&close) {
            cursor = open_index + close_offset + close.len();
        } else {
            cursor = html.len();
            break;
        }
    }
    output.push_str(&html[cursor..]);

    output
}

fn get_token<'a>(
    headers: &'a HeaderMap,
    query_token: Option<&'a str>,
) -> Result<&'a str, (StatusCode, Json<ErrorResponse>)> {
    if let Some(token) = query_token.filter(|token| !token.is_empty()) {
        return Ok(token);
    }

    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or_else(|| public_error(StatusCode::UNAUTHORIZED, "Missing token"))
}

fn selector(value: &str) -> Result<Selector, (StatusCode, Json<ErrorResponse>)> {
    Selector::parse(value)
        .map_err(|_| public_error(StatusCode::INTERNAL_SERVER_ERROR, "Invalid selector"))
}

fn element_text(element: &ElementRef<'_>) -> String {
    element.text().collect::<Vec<_>>().join(" ")
}

fn clean_text(value: &str) -> String {
    value
        .replace('\u{00a0}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn clean_multiline_text(value: &str) -> String {
    let mut lines = Vec::new();
    let mut previous_empty = false;

    for line in value.replace('\u{00a0}', " ").lines().map(|line| {
        line.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }) {
        let is_empty = line.is_empty();
        if is_empty && previous_empty {
            continue;
        }

        lines.push(line);
        previous_empty = is_empty;
    }

    lines.join("\n").trim().to_string()
}

fn public_error(status: StatusCode, message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: true,
            error_message: Some(message.to_string()),
        }),
    )
}

fn remote_error(error: reqwest::Error) -> (StatusCode, Json<ErrorResponse>) {
    eprintln!("Remote info service error: {error}");
    public_error(StatusCode::BAD_GATEWAY, "Failed to fetch remote page")
}
