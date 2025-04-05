use chatbot_rust_wasm::get_response;
use chatbot_rust_wasm::generate_and_store_token;
use chatbot_rust_wasm::persist_data_to_redis;
use std::result::Result;
use urlencoding;
use warp::{http::StatusCode, reply, Reply, Rejection, Buf};
use redis;
use warp::multipart::FormData;
use futures_util::TryStreamExt;
use serde_json;
use serde::{Deserialize};
use base64::Engine;

#[derive(Debug)]
struct MultipartError;
impl warp::reject::Reject for MultipartError {}

#[derive(Deserialize)]
pub struct PhoneQuery {
    pub phone: String,
}

static REDIS_HOST: &str = "127.0.0.1:6379";

pub async fn get_discount(business_name: String, phone_number_amount: String, token: String) -> Result<impl Reply, Rejection> {
    println!("Received get_discount request - Business: {}, Phone/Amount: {}, Token: {}", business_name, phone_number_amount, token);
    let response = match connect_to_redis() {
        Ok(mut conn) => get_response(
            token,
            business_name,
            urlencoding::decode(&phone_number_amount).unwrap_or_default().into_owned(),
            &mut conn,
        ),
        Err(e) => format!("Redis connection failed: {}", e),
    };
    Ok(warp::reply::with_header(
        warp::reply::with_status(response, StatusCode::OK),
        "Cache-Control",
        "no-store, no-cache, must-revalidate, proxy-revalidate",
    ))
}

pub async fn submit_feedback(form: FormData) -> Result<impl Reply, Rejection> {
    let mut rating = String::new();
    let mut note = String::new();
    let mut phone = String::new();
    let mut photo_base64 = None;

    let mut parts = form;
    while let Some(part_result) = parts.try_next().await.map_err(|_| warp::reject::custom(MultipartError))? {
        let mut part = part_result;
        match part.name() {
            "rating" => {
                if let Some(data) = part.data().await {
                    let bytes = data.map_err(|_| warp::reject::custom(MultipartError))?;
                    rating = String::from_utf8(bytes.chunk().to_vec())
                        .map_err(|_| warp::reject::custom(MultipartError))?;
                }
            }
            "note" => {
                if let Some(data) = part.data().await {
                    let bytes = data.map_err(|_| warp::reject::custom(MultipartError))?;
                    note = String::from_utf8(bytes.chunk().to_vec())
                        .map_err(|_| warp::reject::custom(MultipartError))?;
                }
            }
            "phone" => {
                if let Some(data) = part.data().await {
                    let bytes = data.map_err(|_| warp::reject::custom(MultipartError))?;
                    phone = String::from_utf8(bytes.chunk().to_vec())
                        .map_err(|_| warp::reject::custom(MultipartError))?;
                }
            }
            "photo" => {
                if let Some(data) = part.data().await {
                    let bytes = data.map_err(|_| warp::reject::custom(MultipartError))?;
                    photo_base64 = Some(base64::engine::general_purpose::STANDARD.encode(bytes.chunk()));
                }
            }
            _ => {}
        }
    }

    if rating.is_empty() || note.is_empty() || phone.is_empty() {
        return Ok(reply::with_status(
            reply::json(&"Missing rating, note, or phone"),
            StatusCode::BAD_REQUEST,
        ));
    }

    let mut conn = connect_to_redis().map_err(|_| warp::reject::custom(MultipartError))?;
    let feedback_key = format!("feedback:{}:{}", phone, chrono::Utc::now().timestamp());
    let feedback_data = serde_json::json!({
        "rating": rating,
        "note": note,
        "photo": photo_base64.unwrap_or_default(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "phone_number": phone
    });

    persist_data_to_redis(&feedback_key, serde_json::to_string(&feedback_data).unwrap(), &mut conn);

    Ok(reply::with_status(
        reply::json(&"Feedback received and stored!"),
        StatusCode::OK,
    ))
}

pub async fn generate_token(query: PhoneQuery) -> Result<impl Reply, Rejection> {
    let mut conn = connect_to_redis().map_err(|_| warp::reject::custom(MultipartError))?;
    let business_name = "test102".to_string(); // Hardcoded for now
    let token = generate_and_store_token(&query.phone, &business_name, &mut conn);
    println!("Generated token for phone {}: {}", query.phone, token);
    Ok(reply::with_status(
        reply::json(&serde_json::json!({"token": token})),
        StatusCode::OK,
    ))
}

fn connect_to_redis() -> Result<redis::Connection, redis::RedisError> {
    let redis_conn_url = format!("redis://{}", REDIS_HOST);
    let client = redis::Client::open(redis_conn_url)?;
    client.get_connection()
}
