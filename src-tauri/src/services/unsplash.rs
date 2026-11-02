//! Unsplash 搜索与图片下载。

use std::path::Path;

use serde::Deserialize;

use crate::models::{Photo, SearchResponse};

#[derive(Debug, Deserialize)]
struct ApiSearchResponse {
    total: u32,
    total_pages: u32,
    results: Vec<ApiPhoto>,
}

#[derive(Debug, Deserialize)]
struct ApiPhoto {
    id: String,
    width: u32,
    height: u32,
    color: Option<String>,
    description: Option<String>,
    alt_description: Option<String>,
    urls: ApiUrls,
    user: ApiUser,
    links: Option<ApiLinks>,
}

#[derive(Debug, Deserialize)]
struct ApiUrls {
    raw: String,
    full: String,
    regular: String,
    small: String,
}

#[derive(Debug, Deserialize)]
struct ApiUser {
    name: String,
    links: Option<ApiUserLinks>,
}

#[derive(Debug, Deserialize)]
struct ApiUserLinks {
    html: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiLinks {
    html: Option<String>,
    #[serde(default)]
    download_location: Option<String>,
}

fn highest_resolution(urls: &ApiUrls) -> String {
    if !urls.raw.is_empty() {
        urls.raw.clone()
    } else if !urls.full.is_empty() {
        urls.full.clone()
    } else if !urls.regular.is_empty() {
        urls.regular.clone()
    } else {
        urls.small.clone()
    }
}

pub async fn search(
    client: &reqwest::Client,
    key: &str,
    query: &str,
    page: u32,
    per_page: u32,
) -> Result<SearchResponse, String> {
    let response = client
        .get("https://api.unsplash.com/search/photos")
        .query(&[
            ("query", query),
            ("page", &page.to_string()),
            ("per_page", &per_page.to_string()),
            ("orientation", "landscape"),
            ("content_filter", "high"),
        ])
        .header("Authorization", format!("Client-ID {key}"))
        .header("Accept-Version", "v1")
        .send()
        .await
        .map_err(|e| format!("网络请求失败：{e}"))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Unsplash 访问密钥无效，请在「设置」中更新 Access Key。".to_string());
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err("Unsplash 请求被拒绝（可能已达速率限制），请稍后再试。".to_string());
    }
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Unsplash 返回 {status}：{}", body.chars().take(180).collect::<String>()));
    }

    let payload: ApiSearchResponse = response
        .json()
        .await
        .map_err(|e| format!("解析 Unsplash 响应失败：{e}"))?;

    let photos = payload
        .results
        .into_iter()
        .map(|photo| {
            let description = photo
                .description
                .clone()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| photo.alt_description.clone());
            Photo {
                id: photo.id,
                description,
                width: photo.width,
                height: photo.height,
                color: photo.color,
                author: photo.user.name,
                author_url: photo.user.links.as_ref().and_then(|l| l.html.clone()),
                thumb_url: photo.urls.small.clone(),
                preview_url: photo.urls.regular.clone(),
                full_url: highest_resolution(&photo.urls),
                unsplash_url: photo.links.as_ref().and_then(|l| l.html.clone()),
                download_location: photo.links.as_ref().and_then(|l| l.download_location.clone()),
            }
        })
        .collect();

    Ok(SearchResponse {
        query: query.to_string(),
        page,
        total: payload.total,
        total_pages: payload.total_pages,
        photos,
    })
}

/// Unsplash 的 raw 链接可以追加尺寸参数，避免下载原图几十 MB。
pub fn sized_url(url: &str, width: u32) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}w={width}&q=95&fm=jpg&fit=max")
}

pub async fn download(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建下载目录失败：{e}"))?;
    }
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载图片失败：{e}"))?;

    if !response.status().is_success() {
        return Err(format!("下载图片失败：HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取图片数据失败：{e}"))?;

    if bytes.len() < 1024 {
        return Err("下载的图片数据不完整。".to_string());
    }

    std::fs::write(dest, &bytes).map_err(|e| format!("保存图片失败：{e}"))?;
    Ok(())
}

/// 通知 Unsplash 已经下载（遵守其 API 使用规范），失败不影响主流程。
pub fn track_download(client: reqwest::Client, key: String, location: String) {
    if location.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let _ = client
            .get(location)
            .header("Authorization", format!("Client-ID {key}"))
            .send()
            .await;
    });
}
