use actix_cors::Cors;
use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    web, App, Error, HttpResponse, HttpServer, Responder, Either, middleware::Logger,
};
use actix_multipart::Multipart;
use futures_util::stream::StreamExt as _;
use redis::Commands;
use serde::{Deserialize, Serialize};
use std::future::{ready, Ready};

// Custom middleware to log incoming requests
pub struct RequestLogger;

impl<S: 'static, B> Transform<S, ServiceRequest> for RequestLogger
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestLoggerMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestLoggerMiddleware { service }))
    }
}

pub struct RequestLoggerMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestLoggerMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = S::Future;

    fn poll_ready(&self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        println!(
            "Incoming request: {} {} Content-Type: {:?}",
            req.method(),
            req.path(),
            req.headers().get("Content-Type")
        );
        self.service.call(req)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Feedback {
    phone_number: String,
    rating: u8, // 1 to 5 stars
    comment: String,
    photo: Option<String>, // Add photo field as an optional base64 string
}

#[derive(Deserialize)]
struct TokenQuery {
    phone: String,
}

#[derive(Serialize)]
struct TokenResponse {
    token: String,
}

async fn get_discount(
    path: web::Path<(String, String, String)>,
    redis_conn: web::Data<redis::Client>,
) -> impl Responder {
    let (business_name, phone_number_amount, token) = path.into_inner();
    let mut conn = redis_conn
        .get_connection()
        .expect("Failed to get Redis connection");
    let response = chatbot_rust_wasm::get_response(token, business_name, phone_number_amount, &mut conn);
    HttpResponse::Ok().body(response)
}

async fn generate_token(
    query: web::Query<TokenQuery>,
    redis_conn: web::Data<redis::Client>,
) -> impl Responder {
    let mut conn = redis_conn
        .get_connection()
        .expect("Failed to get Redis connection");

    let business_name = "test102";
    let new_token = chatbot_rust_wasm::generate_and_store_token(&query.phone, business_name, &mut conn);

    let response = TokenResponse {
        token: new_token,
    };
    HttpResponse::Ok().json(response)
}

async fn submit_feedback(
    payload: Either<web::Json<Feedback>, Multipart>,
    redis_conn: web::Data<redis::Client>,
) -> impl Responder {
    println!("Received feedback submission request");

    let feedback = match payload {
        Either::Left(json) => {
            println!("Processing JSON payload: {:?}", json);
            json.into_inner()
        }
        Either::Right(mut multipart) => {
            println!("Processing multipart/form-data payload");
            let mut phone_number = String::new();
            let mut rating = 0;
            let mut comment = String::new();
            let mut photo = None;

            // Parse multipart form data
            while let Some(item) = multipart.next().await {
                let mut field = match item {
                    Ok(field) => field,
                    Err(e) => {
                        println!("Failed to parse multipart field: {}", e);
                        return HttpResponse::BadRequest().body(format!("Failed to parse multipart: {}", e));
                    }
                };

                let field_name = field.name().to_string();
                println!("Processing field: {}", field_name);

                if field_name == "phone_number" {
                    while let Some(chunk) = field.next().await {
                        let data = match chunk {
                            Ok(data) => data,
                            Err(e) => {
                                println!("Error reading phone_number chunk: {}", e);
                                return HttpResponse::BadRequest().body("Failed to read phone_number");
                            }
                        };
                        phone_number.push_str(&String::from_utf8_lossy(&data));
                    }
                    println!("Parsed phone_number: {}", phone_number);
                } else if field_name == "rating" {
                    let mut rating_str = String::new();
                    while let Some(chunk) = field.next().await {
                        let data = match chunk {
                            Ok(data) => data,
                            Err(e) => {
                                println!("Error reading rating chunk: {}", e);
                                return HttpResponse::BadRequest().body("Failed to read rating");
                            }
                        };
                        rating_str.push_str(&String::from_utf8_lossy(&data));
                    }
                    rating = rating_str.parse().unwrap_or(0);
                    println!("Parsed rating: {}", rating);
                } else if field_name == "comment" {
                    while let Some(chunk) = field.next().await {
                        let data = match chunk {
                            Ok(data) => data,
                            Err(e) => {
                                println!("Error reading comment chunk: {}", e);
                                return HttpResponse::BadRequest().body("Failed to read comment");
                            }
                        };
                        comment.push_str(&String::from_utf8_lossy(&data));
                    }
                    println!("Parsed comment: {}", comment);
                } else if field_name == "photo" {
                    let mut photo_data = Vec::new();
                    while let Some(chunk) = field.next().await {
                        let data = match chunk {
                            Ok(data) => data,
                            Err(e) => {
                                println!("Error reading photo chunk: {}", e);
                                return HttpResponse::BadRequest().body("Failed to read photo");
                            }
                        };
                        photo_data.extend_from_slice(&data);
                    }
                    // Convert photo to base64
                    photo = Some(base64::encode(&photo_data));
                    println!("Parsed photo as base64 (length: {})", photo.as_ref().unwrap().len());
                }
            }

            Feedback {
                phone_number,
                rating,
                comment,
                photo,
            }
        }
    };

    // Validate rating (1 to 5)
    if feedback.rating < 1 || feedback.rating > 5 {
        println!("Invalid rating: {}", feedback.rating);
        return HttpResponse::BadRequest().body("Rating must be between 1 and 5");
    }

    // Store feedback in Redis
    let mut conn = redis_conn
        .get_connection()
        .expect("Failed to get Redis connection");

    let timestamp = chrono::Utc::now().timestamp();
    let feedback_key = format!("feedback:{}:{}", feedback.phone_number, timestamp);

    let feedback_data = serde_json::to_string(&feedback).unwrap();
    let _: () = conn
        .set(&feedback_key, &feedback_data)
        .expect("Failed to store feedback in Redis");

    println!(
        "Stored feedback - Key: {}, Data: {}",
        feedback_key, feedback_data
    );

    HttpResponse::Ok().body("Feedback submitted successfully!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let redis_client = redis::Client::open("redis://127.0.0.1:6379/").expect("Failed to connect to Redis");

    println!("Server starting on http://0.0.0.0:3030");
    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://127.0.0.1:8000")
            .allowed_methods(vec!["GET", "POST", "OPTIONS"])
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(Logger::default()) // Add default Actix Web logger
            .wrap(RequestLogger) // Add custom request logger
            .app_data(web::Data::new(redis_client.clone()))
            // Configure payload size limit for the entire app (10 MB)
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024)) // 10 MB limit
            .route(
                "/get_discount/{business_name}/phone_number_amount/{phone_number_amount}/token/{token}",
                web::get().to(get_discount),
            )
            .route("/generate_token", web::get().to(generate_token))
            .route("/submit_feedback", web::post().to(submit_feedback))
    })
    .bind("0.0.0.0:3030")?
    .run()
    .await
}
