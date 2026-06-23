use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
    response::Response,
};
use encoding_rs::EUC_KR;
use mongodb::bson::doc;
use reqwest::{Client, StatusCode, Url, header::COOKIE};
use scraper::{ElementRef, Html, Node, Selector};
use serde::{Deserialize, Serialize};

use crate::{
    api::{remote_client, remote_client_builder},
    errors::{ErrorResponse, internal_error},
    state::GlobalState,
};

#[derive(Deserialize)]
pub struct AuthQuery {
    token: Option<String>,
}

#[derive(Deserialize)]
pub struct PostListQuery {
    token: Option<String>,
    gid: String,
    bid: String,
    #[serde(default = "default_board_type")]
    board_type: String,
    page: Option<u32>,
    bucket: Option<u32>,
    s_field: Option<String>,
    s_key: Option<String>,
}

#[derive(Deserialize)]
pub struct PostDetailQuery {
    token: Option<String>,
    gid: String,
    bid: String,
    #[serde(default = "default_board_type")]
    board_type: String,
    page: Option<u32>,
    s_field: Option<String>,
    s_key: Option<String>,
}

#[derive(Deserialize)]
pub struct ImageProxyQuery {
    token: Option<String>,
    url: String,
}

#[derive(Serialize)]
pub struct BoardsResult {
    error: bool,
    error_message: Option<String>,
    boards: Vec<Board>,
}

#[derive(Serialize)]
pub struct Board {
    title: String,
    board_type: String,
    gid: Option<String>,
    bid: Option<String>,
    href: String,
    supported: bool,
}

#[derive(Serialize)]
pub struct PostsResult {
    error: bool,
    error_message: Option<String>,
    gid: String,
    bid: String,
    board_type: String,
    page: u32,
    bucket: u32,
    posts: Vec<PostSummary>,
}

#[derive(Serialize)]
pub struct PostSummary {
    cid: String,
    number: String,
    writer: String,
    title: String,
    registered_at: String,
    views: Option<u32>,
    has_file: bool,
    is_notice: bool,
    is_new: bool,
}

#[derive(Serialize)]
pub struct PostResult {
    error: bool,
    error_message: Option<String>,
    post: PostDetail,
}

#[derive(Serialize)]
pub struct PostTextResult {
    error: bool,
    error_message: Option<String>,
    post: PostDetail,
    text: String,
}

#[derive(Serialize)]
pub struct PostContentResult {
    error: bool,
    error_message: Option<String>,
    post: PostDetail,
    format: String,
    content: String,
}

#[derive(Serialize)]
pub struct PostDetail {
    gid: String,
    bid: String,
    cid: String,
    board_type: String,
    board_title: Option<String>,
    title: Option<String>,
    writer: Option<String>,
    writer_id: Option<String>,
    registered_at: Option<String>,
    views: Option<u32>,
    attachments: Vec<Attachment>,
    content_text: String,
    content_html: String,
}

#[derive(Serialize)]
pub struct Attachment {
    index: Option<u32>,
    name: String,
    size: Option<String>,
    downloads: Option<u32>,
}

pub async fn boards(
    State(state): State<GlobalState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Result<Json<BoardsResult>, (StatusCode, Json<ErrorResponse>)> {
    let token = get_token(&headers, query.token.as_deref())?;
    let session_cookie = get_session_cookie(&state, token).await?;
    let client = remote_client().map_err(internal_error)?;
    let url = format!(
        "https://cyber.{}/Cyber/ComBoard_V005/m_left.jsp",
        state.remote_api_domain
    );
    let html = fetch_html(&client, &url, &session_cookie).await?;
    let boards = parse_boards(&state.remote_api_domain, &html)?;

    Ok(Json(BoardsResult {
        error: false,
        error_message: None,
        boards,
    }))
}

pub async fn posts(
    State(state): State<GlobalState>,
    headers: HeaderMap,
    Query(query): Query<PostListQuery>,
) -> Result<Json<PostsResult>, (StatusCode, Json<ErrorResponse>)> {
    let token = get_token(&headers, query.token.as_deref())?;
    let session_cookie = get_session_cookie(&state, token).await?;
    let client = remote_client().map_err(internal_error)?;
    let page = query.page.unwrap_or(1);
    let bucket = query.bucket.unwrap_or(9);
    let page_string = page.to_string();
    let bucket_string = bucket.to_string();
    let html = fetch_html_with_query(
        &client,
        &board_content_url(&state.remote_api_domain, &query.board_type, "list.jsp")?,
        &session_cookie,
        &[
            ("gid", query.gid.as_str()),
            ("bid", query.bid.as_str()),
            ("lpage", page_string.as_str()),
            ("bucket", bucket_string.as_str()),
            ("sField", query.s_field.as_deref().unwrap_or("")),
            ("sKey", query.s_key.as_deref().unwrap_or("")),
        ],
    )
    .await?;

    Ok(Json(PostsResult {
        error: false,
        error_message: None,
        gid: query.gid,
        bid: query.bid,
        board_type: normalize_board_type(&query.board_type)?,
        page,
        bucket,
        posts: parse_posts(&html)?,
    }))
}

pub async fn post(
    State(state): State<GlobalState>,
    headers: HeaderMap,
    Path(cid): Path<String>,
    Query(query): Query<PostDetailQuery>,
) -> Result<Json<PostResult>, (StatusCode, Json<ErrorResponse>)> {
    let post = fetch_post_detail(&state, &headers, &cid, &query).await?;

    Ok(Json(PostResult {
        error: false,
        error_message: None,
        post,
    }))
}

pub async fn post_text(
    State(state): State<GlobalState>,
    headers: HeaderMap,
    Path(cid): Path<String>,
    Query(query): Query<PostDetailQuery>,
) -> Result<Json<PostTextResult>, (StatusCode, Json<ErrorResponse>)> {
    let post = fetch_post_detail(&state, &headers, &cid, &query).await?;

    Ok(Json(post_text_result_from_detail(post)))
}

pub async fn post_content(
    State(state): State<GlobalState>,
    headers: HeaderMap,
    Path(cid): Path<String>,
    Query(query): Query<PostDetailQuery>,
) -> Result<Json<PostContentResult>, (StatusCode, Json<ErrorResponse>)> {
    let post = fetch_post_detail(&state, &headers, &cid, &query).await?;
    let content = post.content_html.clone();

    Ok(Json(PostContentResult {
        error: false,
        error_message: None,
        post,
        format: "html".to_string(),
        content,
    }))
}

pub async fn post_image(
    State(state): State<GlobalState>,
    headers: HeaderMap,
    Path(_cid): Path<String>,
    Query(query): Query<ImageProxyQuery>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let token = get_token(&headers, query.token.as_deref())?;
    let session_cookie = get_session_cookie(&state, token).await?;
    let url = validate_remote_image_url(&state.remote_api_domain, &query.url)?;
    let client = remote_client_builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(internal_error)?;
    let response = client
        .get(url)
        .header(COOKIE, session_cookie)
        .send()
        .await
        .map_err(remote_error)?;

    if !response.status().is_success() {
        return Err(public_error(
            StatusCode::BAD_GATEWAY,
            "Failed to fetch remote image",
        ));
    }

    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    if !content_type
        .as_ref()
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("image/"))
    {
        return Err(public_error(
            StatusCode::BAD_GATEWAY,
            "Remote resource is not an image",
        ));
    }

    let bytes = response.bytes().await.map_err(remote_error)?;
    let mut builder = Response::builder().status(status);
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    builder = builder.header(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string())
            .map_err(|_| public_error(StatusCode::INTERNAL_SERVER_ERROR, "Invalid header"))?,
    );

    builder.body(Body::from(bytes)).map_err(|_| {
        public_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to build response",
        )
    })
}

pub async fn attachment(
    State(state): State<GlobalState>,
    headers: HeaderMap,
    Path((cid, index)): Path<(String, u32)>,
    Query(query): Query<PostDetailQuery>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let token = get_token(&headers, query.token.as_deref())?;
    let session_cookie = get_session_cookie(&state, token).await?;
    let client = remote_client().map_err(internal_error)?;
    let board_type = normalize_board_type(&query.board_type)?;
    let page = query.page.unwrap_or(1);
    let page_string = page.to_string();
    let index_string = index.to_string();
    let url = format!(
        "https://cyber.{}/Cyber/ComBoardDownLoad",
        state.remote_api_domain
    );
    let response = client
        .post(url)
        .header(COOKIE, session_cookie)
        .form(&[
            ("gubun", ""),
            ("lpage", page_string.as_str()),
            ("gid", query.gid.as_str()),
            ("bid", query.bid.as_str()),
            ("cid", cid.as_str()),
            ("sField", query.s_field.as_deref().unwrap_or("")),
            ("sKey", query.s_key.as_deref().unwrap_or("")),
            ("fchange", "0"),
            ("fgubun", index_string.as_str()),
            ("baseSavePath", board_upload_base_path(&board_type)),
            ("isRealPath", "T"),
        ])
        .send()
        .await
        .map_err(remote_error)?;

    if !response.status().is_success() {
        return Err(public_error(
            StatusCode::BAD_GATEWAY,
            "Failed to download remote attachment",
        ));
    }

    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let content_disposition = response
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .and_then(normalize_content_disposition);
    let bytes = response.bytes().await.map_err(remote_error)?;

    let mut builder = Response::builder().status(status);
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    if let Some(content_disposition) = content_disposition {
        builder = builder.header(header::CONTENT_DISPOSITION, content_disposition);
    }
    builder = builder.header(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string())
            .map_err(|_| public_error(StatusCode::INTERNAL_SERVER_ERROR, "Invalid header"))?,
    );

    builder.body(Body::from(bytes)).map_err(|_| {
        public_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to build response",
        )
    })
}

async fn fetch_post_detail(
    state: &GlobalState,
    headers: &HeaderMap,
    cid: &str,
    query: &PostDetailQuery,
) -> Result<PostDetail, (StatusCode, Json<ErrorResponse>)> {
    let token = get_token(headers, query.token.as_deref())?;
    let session_cookie = get_session_cookie(state, token).await?;
    let client = remote_client().map_err(internal_error)?;
    let page = query.page.unwrap_or(1);
    let page_string = page.to_string();
    let html = fetch_html_with_query(
        &client,
        &board_content_url(&state.remote_api_domain, &query.board_type, "view.jsp")?,
        &session_cookie,
        &[
            ("gid", query.gid.as_str()),
            ("bid", query.bid.as_str()),
            ("cid", cid),
            ("lpage", page_string.as_str()),
            ("sField", query.s_field.as_deref().unwrap_or("")),
            ("sKey", query.s_key.as_deref().unwrap_or("")),
        ],
    )
    .await?;

    parse_post_detail(
        query.gid.clone(),
        query.bid.clone(),
        cid.to_string(),
        normalize_board_type(&query.board_type)?,
        &html,
        &state.remote_api_domain,
        token,
    )
}

fn post_text_result_from_detail(mut post: PostDetail) -> PostTextResult {
    let text = post.content_text.clone();
    post.content_html.clear();

    PostTextResult {
        error: false,
        error_message: None,
        post,
        text,
    }
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

async fn fetch_html(
    client: &Client,
    url: &str,
    session_cookie: &str,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    fetch_html_with_query(client, url, session_cookie, &[]).await
}

async fn fetch_html_with_query(
    client: &Client,
    url: &str,
    session_cookie: &str,
    query: &[(&str, &str)],
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let response = client
        .get(url)
        .query(query)
        .header(COOKIE, session_cookie)
        .send()
        .await
        .map_err(remote_error)?;

    if !response.status().is_success() {
        return Err(public_error(
            StatusCode::BAD_GATEWAY,
            "Failed to fetch remote board",
        ));
    }

    let bytes = response.bytes().await.map_err(remote_error)?;
    let (text, _, _) = EUC_KR.decode(&bytes);
    Ok(text.into_owned())
}

fn parse_boards(
    remote_api_domain: &str,
    html: &str,
) -> Result<Vec<Board>, (StatusCode, Json<ErrorResponse>)> {
    let document = Html::parse_document(html);
    let link_selector = selector("a")?;
    let base_url = format!("https://cyber.{remote_api_domain}");
    let mut in_bbs = false;
    let mut boards = Vec::new();

    for link in document.select(&link_selector) {
        let title = clean_text(&element_text(&link));
        if title == "봉황 BBS" {
            in_bbs = true;
            continue;
        }
        if in_bbs && title == "묻고답하기" {
            break;
        }
        if !in_bbs || title.is_empty() {
            continue;
        }

        let Some(href) = link.value().attr("href") else {
            continue;
        };
        if href == "#" {
            continue;
        }

        let absolute =
            Url::parse(href).or_else(|_| Url::parse(&base_url).and_then(|base| base.join(href)));
        let Ok(url) = absolute else {
            boards.push(Board {
                title,
                board_type: "external".to_string(),
                gid: None,
                bid: None,
                href: href.to_string(),
                supported: false,
            });
            continue;
        };

        let board_type = board_type_from_path(url.path());
        let gid = query_param(&url, "gid");
        let bid = query_param(&url, "bid");
        let supported = board_type != "external" && gid.is_some() && bid.is_some();

        boards.push(Board {
            title,
            board_type,
            gid,
            bid,
            href: url.to_string(),
            supported,
        });
    }

    Ok(boards)
}

fn parse_posts(html: &str) -> Result<Vec<PostSummary>, (StatusCode, Json<ErrorResponse>)> {
    let document = Html::parse_document(html);
    let row_selector = selector("table.table tr")?;
    let cell_selector = selector("td")?;
    let link_selector = selector("a")?;
    let mut posts = Vec::new();

    for row in document.select(&row_selector) {
        let cells = row.select(&cell_selector).collect::<Vec<_>>();
        if cells.len() < 6 {
            continue;
        }

        let Some(link) = cells[2].select(&link_selector).next() else {
            continue;
        };
        let Some(href) = link.value().attr("href") else {
            continue;
        };
        let Some(cid) = parse_js_first_arg(href, "viewGo") else {
            continue;
        };

        let number = clean_text(&element_text(&cells[0]));
        let has_file = !clean_text(&element_text(&cells[5])).is_empty();
        let row_class = row.value().attr("class").unwrap_or("");

        posts.push(PostSummary {
            cid,
            number: number.clone(),
            writer: clean_text(&element_text(&cells[1])),
            title: clean_text(&element_text(&link)),
            registered_at: clean_text(&element_text(&cells[3])),
            views: clean_text(&element_text(&cells[4])).parse::<u32>().ok(),
            has_file,
            is_notice: number.contains("공지") || row_class.contains("notice"),
            is_new: number.eq_ignore_ascii_case("new"),
        });
    }

    Ok(posts)
}

fn parse_post_detail(
    gid: String,
    bid: String,
    cid: String,
    board_type: String,
    html: &str,
    remote_api_domain: &str,
    token: &str,
) -> Result<PostDetail, (StatusCode, Json<ErrorResponse>)> {
    let document = Html::parse_document(html);
    let body_selector = selector("body")?;
    let board_title_selector = selector("h3")?;
    let content_selector = selector("table.bbs-form-2017")?;
    let title_selector = selector("h1")?;
    let row_selector = selector("div.row-fluid")?;
    let body = document
        .select(&body_selector)
        .next()
        .ok_or_else(|| public_error(StatusCode::BAD_GATEWAY, "Failed to parse remote board"))?;

    let content = document.select(&content_selector).next();
    let title = content
        .as_ref()
        .and_then(|content| content.select(&title_selector).next())
        .map(|title| clean_text(&element_text(&title)));
    let post_body = parse_post_body(content, body, remote_api_domain, &cid, token)?;

    let board_title = document
        .select(&board_title_selector)
        .next()
        .map(|title| clean_text(&element_text(&title)));
    let rows = document.select(&row_selector).collect::<Vec<_>>();
    let meta_line = rows
        .iter()
        .map(element_text)
        .map(|text| clean_text(&text))
        .find(|line| line.contains("글쓴이 :"));
    let (writer, writer_id, views, registered_at) = meta_line
        .as_deref()
        .map(parse_meta_line)
        .unwrap_or((None, None, None, None));

    Ok(PostDetail {
        gid,
        bid,
        cid,
        board_type,
        board_title,
        title,
        writer,
        writer_id,
        registered_at,
        views,
        attachments: parse_attachments(&rows)?,
        content_text: post_body.text,
        content_html: post_body.html,
    })
}

struct ParsedPostBody {
    text: String,
    html: String,
}

#[derive(Default)]
struct SanitizedFragment {
    html: String,
    text: String,
}

struct SanitizerContext<'a> {
    remote_api_domain: &'a str,
    cid: &'a str,
    token: &'a str,
    base_url: Url,
}

impl SanitizedFragment {
    fn is_empty(&self) -> bool {
        self.html.trim().is_empty() && clean_text(&self.text).is_empty()
    }

    fn append(&mut self, other: SanitizedFragment) {
        self.html.push_str(&other.html);
        self.text.push_str(&other.text);
    }

    fn append_body_fragment(&mut self, other: SanitizedFragment) {
        if other.is_empty() {
            return;
        }
        if !self.is_empty() {
            self.text.push('\n');
        }
        self.append(other);
    }

    fn push_text(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }
        self.html.push_str(&escape_html(value));
        self.text.push_str(value);
    }

    fn push_break(&mut self) {
        self.html.push_str("<br>");
        self.text.push('\n');
    }
}

impl<'a> SanitizerContext<'a> {
    fn new(
        remote_api_domain: &'a str,
        cid: &'a str,
        token: &'a str,
    ) -> Result<Self, (StatusCode, Json<ErrorResponse>)> {
        Ok(Self {
            remote_api_domain,
            cid,
            token,
            base_url: cyber_base_url(remote_api_domain)?,
        })
    }
}

fn parse_post_body(
    content: Option<ElementRef<'_>>,
    fallback_body: ElementRef<'_>,
    remote_api_domain: &str,
    cid: &str,
    token: &str,
) -> Result<ParsedPostBody, (StatusCode, Json<ErrorResponse>)> {
    let context = SanitizerContext::new(remote_api_domain, cid, token)?;
    let fragment = content
        .map(|content| sanitize_board_table_body(content, &context))
        .unwrap_or_else(|| sanitize_children(&fallback_body, &context));

    Ok(ParsedPostBody {
        text: clean_multiline_text(&fragment.text),
        html: fragment.html.trim().to_string(),
    })
}

fn sanitize_board_table_body(
    table: ElementRef<'_>,
    context: &SanitizerContext<'_>,
) -> SanitizedFragment {
    let mut body = SanitizedFragment::default();
    let mut saw_row = false;

    for child in table.child_elements() {
        match child.value().name() {
            "tr" => {
                saw_row = true;
                append_board_row_body(&mut body, child, context);
            }
            "tbody" | "thead" | "tfoot" => {
                for row in child
                    .child_elements()
                    .filter(|element| element.value().name() == "tr")
                {
                    saw_row = true;
                    append_board_row_body(&mut body, row, context);
                }
            }
            _ => {}
        }
    }

    if !saw_row || body.is_empty() {
        sanitize_children(&table, context)
    } else {
        body
    }
}

fn append_board_row_body(
    body: &mut SanitizedFragment,
    row: ElementRef<'_>,
    context: &SanitizerContext<'_>,
) {
    if is_board_chrome_element(&row) {
        return;
    }

    let mut row_body = SanitizedFragment::default();
    let mut saw_cell = false;

    for cell in row
        .child_elements()
        .filter(|element| matches!(element.value().name(), "td" | "th"))
    {
        saw_cell = true;
        if is_board_chrome_element(&cell) {
            continue;
        }
        row_body.append_body_fragment(sanitize_children(&cell, context));
    }

    if !saw_cell {
        row_body.append_body_fragment(sanitize_children(&row, context));
    }

    body.append_body_fragment(row_body);
}

fn sanitize_children(
    element: &ElementRef<'_>,
    context: &SanitizerContext<'_>,
) -> SanitizedFragment {
    let mut fragment = SanitizedFragment::default();

    for child in element.children() {
        match child.value() {
            Node::Text(text) => fragment.push_text(text),
            Node::Element(_) => {
                if let Some(element) = ElementRef::wrap(child) {
                    fragment.append(sanitize_element(element, context));
                }
            }
            _ => {}
        }
    }

    fragment
}

fn sanitize_element(element: ElementRef<'_>, context: &SanitizerContext<'_>) -> SanitizedFragment {
    let tag_name = element.value().name().to_ascii_lowercase();

    if is_skipped_html_tag(&tag_name) || is_board_chrome_element(&element) {
        return SanitizedFragment::default();
    }

    if tag_name == "br" {
        let mut fragment = SanitizedFragment::default();
        fragment.push_break();
        return fragment;
    }

    if tag_name == "img" {
        return sanitize_image_element(element, context);
    }

    if !is_allowed_html_tag(&tag_name) {
        return sanitize_children(&element, context);
    }

    let mut fragment = SanitizedFragment::default();
    let is_block = is_block_html_tag(&tag_name);
    if is_block {
        fragment.text.push('\n');
    }

    fragment.html.push('<');
    fragment.html.push_str(&tag_name);
    append_safe_attributes(&mut fragment.html, &tag_name, &element, context);
    fragment.html.push('>');
    fragment.append(sanitize_children(&element, context));
    fragment.html.push_str("</");
    fragment.html.push_str(&tag_name);
    fragment.html.push('>');

    if is_block {
        fragment.text.push('\n');
    }

    fragment
}

fn sanitize_image_element(
    element: ElementRef<'_>,
    context: &SanitizerContext<'_>,
) -> SanitizedFragment {
    let Some(src) = element
        .value()
        .attr("src")
        .and_then(|src| image_proxy_src(src, context))
    else {
        return SanitizedFragment::default();
    };

    let mut fragment = SanitizedFragment::default();
    fragment.html.push_str("<img");
    append_attribute(&mut fragment.html, "src", &src);
    for name in ["alt", "title"] {
        if let Some(value) = element.value().attr(name).filter(|value| !value.is_empty()) {
            append_attribute(&mut fragment.html, name, value);
        }
    }
    fragment.html.push('>');

    if let Some(alt) = element.value().attr("alt") {
        fragment.text.push_str(alt);
    }

    fragment
}

fn append_safe_attributes(
    html: &mut String,
    tag_name: &str,
    element: &ElementRef<'_>,
    context: &SanitizerContext<'_>,
) {
    match tag_name {
        "a" => {
            if let Some(href) = element
                .value()
                .attr("href")
                .and_then(|href| safe_link_href(href, context))
            {
                append_attribute(html, "href", &href);
            }
            for name in ["title"] {
                if let Some(value) = element.value().attr(name).filter(|value| !value.is_empty()) {
                    append_attribute(html, name, value);
                }
            }
        }
        "td" | "th" => {
            for name in ["title", "colspan", "rowspan"] {
                if let Some(value) = element.value().attr(name).filter(|value| {
                    !value.is_empty()
                        && (name == "title"
                            || value.chars().all(|character| character.is_ascii_digit()))
                }) {
                    append_attribute(html, name, value);
                }
            }
        }
        _ => {
            if let Some(value) = element
                .value()
                .attr("title")
                .filter(|value| !value.is_empty())
            {
                append_attribute(html, "title", value);
            }
        }
    }
}

fn append_attribute(html: &mut String, name: &str, value: &str) {
    html.push(' ');
    html.push_str(name);
    html.push_str("=\"");
    html.push_str(&escape_attribute(value));
    html.push('"');
}

fn safe_link_href(value: &str, context: &SanitizerContext<'_>) -> Option<String> {
    let url = resolve_url(value, &context.base_url)?;
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}

fn image_proxy_src(value: &str, context: &SanitizerContext<'_>) -> Option<String> {
    let url = resolve_url(value, &context.base_url)?;
    if !is_remote_cyber_url(context.remote_api_domain, &url) {
        return None;
    }

    Some(format!(
        "/bbs/posts/{}/images?url={}&token={}",
        percent_encode_path_segment(context.cid),
        percent_encode_query_component(url.as_str()),
        percent_encode_query_component(context.token),
    ))
}

fn resolve_url(value: &str, base_url: &Url) -> Option<Url> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    Url::parse(value).or_else(|_| base_url.join(value)).ok()
}

fn validate_remote_image_url(
    remote_api_domain: &str,
    value: &str,
) -> Result<Url, (StatusCode, Json<ErrorResponse>)> {
    let url = Url::parse(value)
        .map_err(|_| public_error(StatusCode::BAD_REQUEST, "Invalid image URL"))?;
    if !is_remote_cyber_url(remote_api_domain, &url) {
        return Err(public_error(
            StatusCode::BAD_REQUEST,
            "Unsupported image URL",
        ));
    }

    Ok(url)
}

fn cyber_base_url(remote_api_domain: &str) -> Result<Url, (StatusCode, Json<ErrorResponse>)> {
    Url::parse(&format!("https://cyber.{remote_api_domain}/"))
        .map_err(|_| public_error(StatusCode::INTERNAL_SERVER_ERROR, "Invalid remote domain"))
}

fn is_remote_cyber_url(remote_api_domain: &str, url: &Url) -> bool {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }

    url.host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case(&format!("cyber.{remote_api_domain}")))
}

fn is_board_chrome_element(element: &ElementRef<'_>) -> bool {
    if element.value().name().eq_ignore_ascii_case("h1") {
        return true;
    }

    let text = clean_text(&element_text(element));
    text.contains("글쓴이 :")
        || (text.contains("NO. :") && text.contains("등록일 :"))
        || text.contains("첨부파일(")
        || text.contains("File size is")
}

fn is_skipped_html_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "script"
            | "style"
            | "noscript"
            | "template"
            | "form"
            | "input"
            | "button"
            | "select"
            | "textarea"
            | "option"
            | "iframe"
            | "object"
            | "embed"
            | "link"
            | "meta"
    )
}

fn is_allowed_html_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "p" | "div"
            | "span"
            | "b"
            | "strong"
            | "i"
            | "em"
            | "u"
            | "ul"
            | "ol"
            | "li"
            | "table"
            | "thead"
            | "tbody"
            | "tr"
            | "td"
            | "th"
            | "a"
            | "img"
            | "br"
    )
}

fn is_block_html_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "p" | "div" | "ul" | "ol" | "li" | "table" | "thead" | "tbody" | "tr"
    )
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn escape_attribute(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn parse_attachments(
    rows: &[ElementRef<'_>],
) -> Result<Vec<Attachment>, (StatusCode, Json<ErrorResponse>)> {
    let link_selector = selector("a")?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let text = clean_text(&element_text(row));
            if !text.contains("첨부파일(") {
                return None;
            }

            let index = between(&text, "첨부파일(", ")").and_then(|value| value.parse().ok());
            let name = row
                .select(&link_selector)
                .find(|link| {
                    link.value()
                        .attr("href")
                        .is_some_and(|href| href.contains("downGo"))
                })
                .map(|link| clean_text(&element_text(&link)))
                .or_else(|| between(&text, " : ", " File size is").map(|value| clean_text(&value)))
                .unwrap_or_default();
            let (size, downloads) = parse_attachment_size_and_downloads(&text);

            Some(Attachment {
                index,
                name,
                size,
                downloads,
            })
        })
        .collect())
}

fn parse_attachment_size_and_downloads(text: &str) -> (Option<String>, Option<u32>) {
    let Some(tail) = after(text, "File size is") else {
        return (None, None);
    };
    let Some(times_index) = tail.find(" Times") else {
        return (Some(clean_text(&tail)), None);
    };
    let before_times = clean_text(&tail[..times_index]);
    let Some((size, downloads)) = before_times.rsplit_once(' ') else {
        return (Some(before_times), None);
    };

    (Some(clean_text(size)), downloads.parse::<u32>().ok())
}

fn parse_meta_line(line: &str) -> (Option<String>, Option<String>, Option<u32>, Option<String>) {
    let writer_raw = between(line, "글쓴이 :", "NO. :").map(|value| clean_text(&value));
    let (writer, writer_id) = writer_raw
        .as_deref()
        .map(split_writer)
        .unwrap_or((None, None));
    let views = between(line, "조회수 :", "등록일 :").and_then(|value| {
        clean_text(&value)
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u32>()
            .ok()
    });
    let registered_at = after(line, "등록일 :").map(|value| clean_text(&value));

    (writer, writer_id, views, registered_at)
}

fn split_writer(value: &str) -> (Option<String>, Option<String>) {
    let Some(open_index) = value.rfind('(') else {
        return (Some(value.to_string()), None);
    };
    let Some(close_index) = value[open_index..].find(')') else {
        return (Some(value.to_string()), None);
    };

    let close_index = open_index + close_index;
    let writer = clean_text(&value[..open_index]);
    let writer_id = clean_text(&value[open_index + 1..close_index]);

    (
        (!writer.is_empty()).then_some(writer),
        (!writer_id.is_empty()).then_some(writer_id),
    )
}

fn board_content_url(
    remote_api_domain: &str,
    board_type: &str,
    file_name: &str,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    Ok(format!(
        "https://cyber.{}/Cyber/{}/Content/{}",
        remote_api_domain,
        board_path(normalize_board_type(board_type)?.as_str())?,
        file_name
    ))
}

fn normalize_board_type(board_type: &str) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    match board_type {
        "com" | "ComBoard" | "ComBoard_V005" => Ok("com".to_string()),
        "co" | "CoBoard" | "CoBoard_V005" => Ok("co".to_string()),
        _ => Err(public_error(StatusCode::BAD_REQUEST, "Invalid board_type")),
    }
}

fn board_path(board_type: &str) -> Result<&'static str, (StatusCode, Json<ErrorResponse>)> {
    match board_type {
        "com" => Ok("ComBoard_V005"),
        "co" => Ok("CoBoard_V005"),
        _ => Err(public_error(StatusCode::BAD_REQUEST, "Invalid board_type")),
    }
}

fn board_upload_base_path(board_type: &str) -> &'static str {
    match board_type {
        "co" => "/wupis/cyber/CoBoard/upload/upload",
        _ => "/wupis/cyber/ComBoard/upload/upload",
    }
}

fn board_type_from_path(path: &str) -> String {
    if path.contains("/ComBoard_V005/") {
        "com".to_string()
    } else if path.contains("/CoBoard_V005/") {
        "co".to_string()
    } else {
        "external".to_string()
    }
}

fn query_param(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find_map(|(name, value)| (name == key).then(|| value.into_owned()))
}

fn parse_js_first_arg(value: &str, function_name: &str) -> Option<String> {
    let start = value.find(function_name)?;
    let open = value[start..].find('(')? + start;
    let close = value[open..].find(')')? + open;
    let args = &value[open + 1..close];
    let first = args.split(',').next()?.trim();

    Some(
        first
            .trim_matches('"')
            .trim_matches('\'')
            .trim()
            .to_string(),
    )
    .filter(|arg| !arg.is_empty())
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

    for line in value.replace('\u{00a0}', " ").lines().map(clean_text) {
        let is_empty = line.is_empty();
        if is_empty && previous_empty {
            continue;
        }

        lines.push(line);
        previous_empty = is_empty;
    }

    lines.join("\n").trim().to_string()
}

fn normalize_content_disposition(value: &HeaderValue) -> Option<HeaderValue> {
    let (decoded, _, _) = EUC_KR.decode(value.as_bytes());
    let filename = filename_from_content_disposition(&decoded)?;
    let fallback = ascii_filename_fallback(&filename);
    let encoded = percent_encode_utf8(&filename);

    HeaderValue::from_str(&format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}"
    ))
    .ok()
}

fn filename_from_content_disposition(value: &str) -> Option<String> {
    value.split(';').find_map(|part| {
        let part = part.trim();
        part.strip_prefix("filename=")
            .map(|filename| filename.trim().trim_matches('"').to_string())
            .filter(|filename| !filename.is_empty())
    })
}

fn ascii_filename_fallback(filename: &str) -> String {
    let fallback = filename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if fallback.is_empty() {
        "attachment".to_string()
    } else {
        fallback
    }
}

fn percent_encode_utf8(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
            )
        {
            encoded.push(*byte as char);
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }

    encoded
}

fn percent_encode_path_segment(value: &str) -> String {
    percent_encode_url_component(value)
}

fn percent_encode_query_component(value: &str) -> String {
    percent_encode_url_component(value)
}

fn percent_encode_url_component(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(*byte as char);
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }

    encoded
}

fn between(value: &str, start: &str, end: &str) -> Option<String> {
    let start_index = value.find(start)? + start.len();
    let tail = &value[start_index..];
    let end_index = tail.find(end)?;
    Some(tail[..end_index].to_string())
}

fn after(value: &str, start: &str) -> Option<String> {
    let start_index = value.find(start)? + start.len();
    Some(value[start_index..].to_string())
}

fn default_board_type() -> String {
    "com".to_string()
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
    eprintln!("Remote board error: {error}");
    public_error(StatusCode::BAD_GATEWAY, "Failed to fetch remote board")
}

#[cfg(test)]
mod tests {
    use super::*;

    const POST_FIXTURE: &str = r#"
        <html>
          <body>
            <h3>학사공지</h3>
            <table class="bbs-form-2017">
              <tr>
                <td><h1>Post title</h1></td>
              </tr>
              <tr>
                <td>
                  <div class="row-fluid">
                    글쓴이 : Alice (alice01) NO. : 7 조회수 : 42 등록일 : 2026-06-23
                  </div>
                </td>
              </tr>
              <tr>
                <td>
                  <div class="post-body" onclick="evil()">
                    Visible body <strong>bold text</strong>
                    <script>alert('script text')</script>
                    <style>.hidden-style { display: none; }</style>
                    <noscript>noscript text</noscript>
                    <form>
                      <input value="secret">
                      <button>submit text</button>
                    </form>
                    <a href="javascript:alert(1)" onclick="evil()">bad link</a>
                    <a href="/Cyber/read.jsp?x=1&amp;y=2" title="safe" onclick="evil()">safe link</a>
                    <img src="/Cyber/upload/pic.png?x=1&amp;y=2" alt="Remote &amp; Pic" onerror="evil()">
                    <img src="https://evil.example/image.png" alt="evil image">
                    <span style="color:red" data-x="1">span text</span>
                    <table onclick="evil()">
                      <tr><td colspan="2" onclick="evil()">Nested cell</td></tr>
                    </table>
                    <iframe>iframe text</iframe>
                    <custom-tag>custom child</custom-tag>
                  </div>
                </td>
              </tr>
              <tr>
                <td>
                  <div class="row-fluid">
                    첨부파일(1) : <a href="javascript:downGo('1')">file.pdf</a> File size is 12 KB 3 Times Download
                  </div>
                </td>
              </tr>
            </table>
          </body>
        </html>
    "#;

    #[test]
    fn post_body_text_excludes_board_chrome_and_unsafe_nodes() {
        let post = parse_fixture_post();

        assert_eq!(post.title.as_deref(), Some("Post title"));
        assert_eq!(post.writer.as_deref(), Some("Alice"));
        assert_eq!(post.writer_id.as_deref(), Some("alice01"));
        assert_eq!(post.views, Some(42));
        assert_eq!(post.attachments.len(), 1);
        assert_eq!(post.attachments[0].name, "file.pdf");
        assert_eq!(post.attachments[0].size.as_deref(), Some("12 KB"));
        assert_eq!(post.attachments[0].downloads, Some(3));

        for expected in [
            "Visible body",
            "bold text",
            "bad link",
            "safe link",
            "Remote & Pic",
            "span text",
            "Nested cell",
            "custom child",
        ] {
            assert!(
                post.content_text.contains(expected),
                "missing text: {expected}\n{}",
                post.content_text
            );
        }

        for unexpected in [
            "Post title",
            "글쓴이",
            "첨부파일",
            "File size",
            "script text",
            "hidden-style",
            "noscript text",
            "secret",
            "submit text",
            "iframe text",
            "evil image",
        ] {
            assert!(
                !post.content_text.contains(unexpected),
                "unexpected text: {unexpected}\n{}",
                post.content_text
            );
        }
    }

    #[test]
    fn post_body_html_is_sanitized_and_uses_image_proxy() {
        let post = parse_fixture_post();
        let html = post.content_html;

        assert!(html.contains("<strong>bold text</strong>"));
        assert!(html.contains("<a>bad link</a>"));
        assert!(html.contains(
            "href=\"https://cyber.example.edu/Cyber/read.jsp?x=1&amp;y=2\" title=\"safe\""
        ));
        assert!(html.contains("<table>"));
        assert!(html.contains("colspan=\"2\""));
        assert!(html.contains("<span>span text</span>"));
        assert!(html.contains("custom child"));
        assert!(html.contains(
            "src=\"/bbs/posts/cid-1/images?url=https%3A%2F%2Fcyber.example.edu%2FCyber%2Fupload%2Fpic.png%3Fx%3D1%26y%3D2&amp;token=local-token\""
        ));

        for unexpected in [
            "<script",
            "<style",
            "<noscript",
            "<form",
            "<input",
            "<button",
            "<iframe",
            "onclick",
            "onerror",
            "style=",
            "data-x",
            "javascript:",
            "evil.example",
            "Post title",
            "글쓴이",
            "첨부파일",
            "File size",
            "script text",
            "hidden-style",
            "noscript text",
            "secret",
            "submit text",
            "iframe text",
        ] {
            assert!(
                !html.contains(unexpected),
                "unexpected HTML: {unexpected}\n{html}"
            );
        }
    }

    #[test]
    fn text_result_preserves_post_envelope_and_legacy_text() {
        let post = parse_fixture_post();
        let content_text = post.content_text.clone();
        let result = post_text_result_from_detail(post);

        assert!(!result.error);
        assert_eq!(result.error_message, None);
        assert_eq!(result.text, content_text);
        assert_eq!(result.post.content_text, content_text);
        assert_eq!(result.post.content_html, "");
        assert_eq!(result.post.cid, "cid-1");
        assert_eq!(result.post.attachments.len(), 1);
    }

    #[test]
    fn image_proxy_validation_rejects_non_cyber_urls() {
        assert!(
            validate_remote_image_url("example.edu", "https://cyber.example.edu/Cyber/pic.png")
                .is_ok()
        );
        assert!(
            validate_remote_image_url("example.edu", "http://cyber.example.edu/Cyber/pic.png")
                .is_err()
        );
        assert!(
            validate_remote_image_url("example.edu", "https://evil.example/Cyber/pic.png").is_err()
        );
        assert!(validate_remote_image_url("example.edu", "javascript:alert(1)").is_err());
    }

    fn parse_fixture_post() -> PostDetail {
        match parse_post_detail(
            "gid1".to_string(),
            "bid1".to_string(),
            "cid-1".to_string(),
            "com".to_string(),
            POST_FIXTURE,
            "example.edu",
            "local-token",
        ) {
            Ok(post) => post,
            Err((status, response)) => panic!(
                "fixture should parse: status={status}, message={:?}",
                response.error_message
            ),
        }
    }
}
