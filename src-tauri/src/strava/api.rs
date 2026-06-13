use crate::errors::AppError;
use crate::strava::types::UploadStatus;
use reqwest::multipart::{Form, Part};
use tokio::time::{sleep, Duration};

const UPLOADS_URL: &str = "https://www.strava.com/api/v3/uploads";

/// Returns the created Strava activity id.
pub async fn upload_tcx(
    access_token: &str,
    tcx: String,
    name: &str,
    description: &str,
) -> Result<i64, AppError> {
    let client = reqwest::Client::new();

    let part = Part::text(tcx)
        .file_name("activity.tcx")
        .mime_str("application/xml")
        .map_err(|e| AppError::StravaUpload(e.to_string()))?;
    let mut form = Form::new()
        .part("file", part)
        .text("data_type", "tcx")
        .text("name", name.to_string())
        .text("trainer", "1")
        .text("activity_type", "VirtualRide");
    if !description.is_empty() {
        form = form.text("description", description.to_string());
    }

    let resp = client
        .post(UPLOADS_URL)
        .bearer_auth(access_token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| AppError::StravaUpload(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::StravaUpload(format!("upload {status}: {body}")));
    }
    let upload: UploadStatus = resp
        .json()
        .await
        .map_err(|e| AppError::StravaUpload(e.to_string()))?;
    if let Some(err) = upload.error {
        return Err(AppError::StravaUpload(err));
    }

    poll_activity_id(&client, access_token, upload.id).await
}

async fn poll_activity_id(
    client: &reqwest::Client,
    access_token: &str,
    upload_id: i64,
) -> Result<i64, AppError> {
    let url = format!("{UPLOADS_URL}/{upload_id}");
    for _ in 0..15 {
        sleep(Duration::from_secs(2)).await;
        let resp = client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AppError::StravaUpload(e.to_string()))?;
        let status: UploadStatus = resp
            .json()
            .await
            .map_err(|e| AppError::StravaUpload(e.to_string()))?;
        if let Some(err) = status.error {
            return Err(AppError::StravaUpload(err));
        }
        if let Some(activity_id) = status.activity_id {
            return Ok(activity_id);
        }
    }
    Err(AppError::StravaUpload(
        "upload still processing after 30s".into(),
    ))
}
