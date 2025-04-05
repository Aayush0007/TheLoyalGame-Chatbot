extern crate lazy_static;
use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use redis;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct CustomerDiscountDetails {
    pub total_pooled_amount: f64,
    pub total_eligible_customers: f64,
    pub total_discount_given: f64,
    pub customer_expense_map: HashMap<String, HashMap<String, String>>,
}

impl Default for CustomerDiscountDetails {
    fn default() -> CustomerDiscountDetails {
        CustomerDiscountDetails {
            total_pooled_amount: 0.0,
            total_eligible_customers: 0.0,
            total_discount_given: 0.0,
            customer_expense_map: HashMap::new(),
        }
    }
}

pub fn get_response(
    token: String,
    business_name: String,
    phone_number_amount: String,
    conn: &mut redis::Connection,
) -> String {
    let now = Utc::now();
    let now_date = now.format("%d-%b-%Y").to_string();
    let token_key = format!("token:{}", token);
    let token_data = fetch_data_from_redis(&token_key, conn);
    let mut token_expiry_date: NaiveDate = now.date_naive() - Duration::days(7);
    let username_token_key = format!("{}_token_{}", business_name, token);
    let verified_token = fetch_data_from_redis(&username_token_key, conn);
    println!(
        "Token validation - Provided: {}, Verified: {}, Token Data: {}",
        token, verified_token, token_data
    );

    let (stored_token, expiry_date_str) = if !token_data.is_empty() {
        let parts: Vec<&str> = token_data.split("___").collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (String::new(), String::new())
        }
    } else {
        (String::new(), String::new())
    };

    if !expiry_date_str.is_empty() {
        match NaiveDate::parse_from_str(&expiry_date_str, "%d-%b-%Y") {
            Ok(date) => token_expiry_date = date,
            Err(e) => {
                eprintln!(
                    "Failed to parse token expiry date '{}': {}",
                    expiry_date_str, e
                );
                return "Not authorized / Invalid token expiry date.".to_string();
            }
        }
    }

    if stored_token != token || verified_token != token || token_expiry_date < now.date_naive() {
        eprintln!(
            "Token validation failed: stored_token='{}', verified_token='{}', token='{}', expiry_date='{}', now='{}'",
            stored_token, verified_token, token, token_expiry_date, now.date_naive()
        );
        return "Not authorized / Token expired.".to_string();
    }

    static DISCOUNT_POOL_PERCENTAGE: f64 = 0.03;
    static REDIS_KEY_SEPARATOR: &str = "___";
    let phone_amount_vec = phone_number_amount.split(",").collect::<Vec<&str>>();
    if phone_amount_vec.len() != 2 {
        return "Invalid phone_number_amount format. Expected 'phone,amount'.".to_string();
    }
    let phone_number_str = phone_amount_vec[0].trim();
    let amount_str = phone_amount_vec[1].trim();
    let re = Regex::new(r"^[0-9]{10}$").unwrap();
    if !re.is_match(phone_number_str) {
        return format!(
            "Invalid phone number: {}. Must be 10 digits.",
            phone_number_str
        );
    }
    let amount_float = amount_str.parse::<f64>().unwrap_or(0.0);
    let phone_number = phone_number_str.parse::<i64>().unwrap_or(0);

    let current_monday_date = get_latest_monday(now.iso_week().week());
    let current_week_redis_key = format!(
        "{}{}{}",
        business_name,
        REDIS_KEY_SEPARATOR,
        current_monday_date.format("%d-%b-%Y").to_string().as_str()
    );
    let current_week_customer_discount_details_str =
        fetch_data_from_redis(&current_week_redis_key, conn);
    let mut current_week_customer_discount_details: CustomerDiscountDetails =
        if current_week_customer_discount_details_str.is_empty() {
            CustomerDiscountDetails::default()
        } else {
            serde_json::from_str(&current_week_customer_discount_details_str).unwrap_or_default()
        };
    println!(
        "Current week data - Key: {}, Data: {}, Parsed: {:?}",
        current_week_redis_key,
        current_week_customer_discount_details_str,
        current_week_customer_discount_details
    );

    let last_monday_date = current_monday_date - Duration::days(7);
    let redis_key = format!(
        "{}{}{}",
        business_name,
        REDIS_KEY_SEPARATOR,
        last_monday_date.format("%d-%b-%Y").to_string().as_str()
    );
    let customer_discount_details_str = fetch_data_from_redis(&redis_key, conn);
    let customer_discount_details: CustomerDiscountDetails =
        if customer_discount_details_str.is_empty() {
            CustomerDiscountDetails::default()
        } else {
            serde_json::from_str(&customer_discount_details_str).unwrap_or_default()
        };
    println!(
        "Previous week data - Key: {}, Data: {}, Parsed: {:?}",
        redis_key,
        customer_discount_details_str,
        customer_discount_details
    );

    let mut discount = 0.0;
    let has_current_week_transaction = current_week_customer_discount_details
        .customer_expense_map
        .get(phone_number_str)
        .map_or(false, |map| map.contains_key(&now_date));
    println!(
        "Discount check - Phone: {}, Has transaction: {}",
        phone_number_str, has_current_week_transaction
    );

    if !has_current_week_transaction {
        if customer_discount_details
            .customer_expense_map
            .contains_key(phone_number_str)
        {
            let total_pooled_amount = customer_discount_details.total_pooled_amount;
            let total_eligible_discountees = customer_discount_details.total_eligible_customers
                + current_week_customer_discount_details.total_eligible_customers;
            println!(
                "Calculating discount - Pooled: {}, Eligible: {}",
                total_pooled_amount, total_eligible_discountees
            );
            if total_eligible_discountees > 0.0 {
                discount = total_pooled_amount / total_eligible_discountees;
                println!("Discount applied: {}", discount);
            }
        }
    }

    let final_amount = amount_float - discount;
    let pooled_amount = final_amount * DISCOUNT_POOL_PERCENTAGE;
    let discount_perc = if amount_float != 0.0 {
        (discount / amount_float) * 100.0
    } else {
        0.0
    };

    let mut current_week_customer_expense_map =
        current_week_customer_discount_details.customer_expense_map;

    let mut current_week_total_eligible_customers =
        current_week_customer_discount_details.total_eligible_customers;

    if current_week_customer_expense_map.contains_key(phone_number_str) {
        let particular_customer_current_week_expense_map = current_week_customer_expense_map
            .get_mut(phone_number_str)
            .unwrap();
        let existing_amounts = particular_customer_current_week_expense_map
            .entry(now_date.clone())
            .or_insert(String::new());
        if existing_amounts.is_empty() {
            *existing_amounts = final_amount.to_string();
        } else {
            *existing_amounts = format!("{},{}", existing_amounts, final_amount);
        }
    } else {
        current_week_total_eligible_customers += 1.0;
        let mut particular_customer_current_week_expense_map: HashMap<String, String> =
            HashMap::new();
        particular_customer_current_week_expense_map.insert(now_date.clone(), final_amount.to_string());
        current_week_customer_expense_map.insert(
            phone_number_str.to_string(),
            particular_customer_current_week_expense_map,
        );
    }

    let mut current_week_total_pooled_amount =
        current_week_customer_discount_details.total_pooled_amount;

    current_week_total_pooled_amount += pooled_amount;

    current_week_customer_discount_details.customer_expense_map = current_week_customer_expense_map;
    current_week_customer_discount_details.total_eligible_customers =
        current_week_total_eligible_customers;
    current_week_customer_discount_details.total_pooled_amount = current_week_total_pooled_amount;
    current_week_customer_discount_details.total_discount_given += discount;
    persist_data_to_redis(
        &current_week_redis_key,
        serde_json::to_string(&current_week_customer_discount_details).unwrap(),
        conn,
    );

    format!(
        "Phone number: {}\n ; Final bill amount: {:.2}\n ; Discount given: {:.2}%",
        phone_number, final_amount, discount_perc
    )
}

pub fn generate_and_store_token(
    phone_number: &str,
    business_name: &str,
    conn: &mut redis::Connection,
) -> String {
    let token = Uuid::new_v4().to_string();
    let expiry_date = (Utc::now() + Duration::days(7))
        .format("%d-%b-%Y")
        .to_string();
    let token_key = format!("token:{}", token);
    let phone_token_key = format!("phone:{}:token", phone_number);
    let business_token_key = format!("{}_token_{}", business_name, token);

    let token_data = format!("{}___{}", token, expiry_date);
    persist_data_to_redis(&token_key, token_data, conn);
    persist_data_to_redis(&phone_token_key, token.clone(), conn);
    persist_data_to_redis(&business_token_key, token.clone(), conn);

    println!(
        "Generated token - Token: {}, Expiry: {}, Token Key: {}, Business Key: {}",
        token, expiry_date, token_key, business_token_key
    );

    token
}

pub fn fetch_data_from_redis(redis_key: &str, conn: &mut redis::Connection) -> String {
    let exists: bool = redis::cmd("EXISTS")
        .arg(redis_key)
        .query(conn)
        .unwrap_or(false);
    if !exists {
        return String::new();
    }
    redis::cmd("GET")
        .arg(redis_key)
        .query(conn)
        .unwrap_or_default()
}

pub fn persist_data_to_redis(redis_key: &str, value: String, conn: &mut redis::Connection) {
    let _: () = redis::cmd("SET")
        .arg(redis_key)
        .arg(value)
        .query(conn)
        .unwrap_or(());
}

fn get_latest_monday(week: u32) -> NaiveDate {
    let current_year = chrono::offset::Local::now().year();
    NaiveDate::from_isoywd_opt(current_year, week, Weekday::Mon).unwrap()
}
#[cfg(test)]
mod test {
    use super::*;
    use chrono::Duration;
    use std::sync::Mutex;

    static REDIS_HOST: &str = "127.0.0.1:6379";
    lazy_static::lazy_static! {
        static ref REDIS_CONNECTION: Mutex<redis::Connection> = {
            let redis_conn_url = format!("redis://{}", REDIS_HOST);
            let conn = redis::Client::open(redis_conn_url)
                .expect("Invalid connection URL")
                .get_connection()
                .expect("failed to connect to Redis");
            Mutex::new(conn)
        };
    }

    fn setup_previous_week_data(
        conn: &mut redis::Connection,
        business_name: &str,
        phone: &str,
        total_pooled_amount: f64,
        total_eligible_customers: f64,
    ) {
        let last_monday = get_latest_monday(Utc::now().iso_week().week()) - Duration::days(7);
        let redis_key = format!("{}___{}", business_name, last_monday.format("%d-%b-%Y"));
        let mut customer_discount_details = CustomerDiscountDetails::default();
        let mut expense_map: HashMap<String, String> = HashMap::new();
        expense_map.insert("10-Mar-2025".to_string(), "1000.00".to_string());
        customer_discount_details.customer_expense_map.insert(phone.to_string(), expense_map);
        customer_discount_details.total_pooled_amount = total_pooled_amount;
        customer_discount_details.total_eligible_customers = total_eligible_customers;
        let serialized_data = serde_json::to_string(&customer_discount_details).unwrap();
        println!(
            "Setting previous week data - Key: {}, Data: {}",
            redis_key, serialized_data
        );
        persist_data_to_redis(&redis_key, serialized_data, conn);
    }

    fn setup_current_week_data(
        conn: &mut redis::Connection,
        business_name: &str,
        phone: &str,
        has_transaction_today: bool,
        total_eligible_customers: f64,
    ) {
        let current_monday = get_latest_monday(Utc::now().iso_week().week());
        let redis_key = format!("{}___{}", business_name, current_monday.format("%d-%b-%Y"));
        let mut customer_discount_details = CustomerDiscountDetails::default();
        if has_transaction_today {
            let mut expense_map: HashMap<String, String> = HashMap::new();
            let today = Utc::now().format("%d-%b-%Y").to_string();
            expense_map.insert(today, "500.00".to_string());
            customer_discount_details.customer_expense_map.insert(phone.to_string(), expense_map);
        }
        customer_discount_details.total_eligible_customers = total_eligible_customers;
        let serialized_data = serde_json::to_string(&customer_discount_details).unwrap();
        println!(
            "Setting current week data - Key: {}, Data: {}",
            redis_key, serialized_data
        );
        persist_data_to_redis(&redis_key, serialized_data, conn);
    }

    #[test]
    fn test_discount_eligible_first_transaction() {
        let mut conn = REDIS_CONNECTION.lock().unwrap();
        // Clear Redis before the test
        let _: () = redis::cmd("FLUSHALL").query(&mut *conn).unwrap();
        let keys_after_flush: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut *conn).unwrap();
        println!("Keys after FLUSHALL: {:?}", keys_after_flush);

        let phone = "9876543210";
        let business_name = "test102";
        let token = generate_and_store_token(phone, business_name, &mut *conn);

        // Setup previous week data
        setup_previous_week_data(&mut *conn, business_name, phone, 30.0, 1.0);

        // Explicitly set current week data with total_eligible_customers = 0.0
        setup_current_week_data(&mut *conn, business_name, "different_phone", false, 0.0);

        let result = get_response(
            token,
            business_name.to_string(),
            format!("{}, 678.90", phone),
            &mut *conn,
        );
        println!("Test discount_eligible_first_transaction: {}", result);
        // Expected discount: 30.0 / (1.0 + 0.0) = 30.0
        // Final bill: 678.90 - 30.0 = 648.90
        // Discount percentage: (30.0 / 678.90) * 100 ≈ 4.42%
        assert!(result.contains("Final bill amount: 648.90"));
        assert!(result.contains("Discount given: 4.42"));
    }

    #[test]
    fn test_discount_not_eligible_already_received() {
        let mut conn = REDIS_CONNECTION.lock().unwrap();
        // Clear Redis before the test
        let _: () = redis::cmd("FLUSHALL").query(&mut *conn).unwrap();
        let keys_after_flush: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut *conn).unwrap();
        println!("Keys after FLUSHALL: {:?}", keys_after_flush);

        let phone = "9876543210";
        let business_name = "test102";
        let token = generate_and_store_token(phone, business_name, &mut *conn);

        // Setup previous week data
        setup_previous_week_data(&mut *conn, business_name, phone, 30.0, 1.0);

        // Setup current week data with a transaction today
        setup_current_week_data(&mut *conn, business_name, phone, true, 1.0);

        let result = get_response(
            token,
            business_name.to_string(),
            format!("{}, 678.90", phone),
            &mut *conn,
        );
        println!("Test discount_not_eligible_already_received: {}", result);
        // Expected: No discount since the customer already has a transaction today
        // Final bill: 678.90 - 0.0 = 678.90
        // Discount percentage: 0.0%
        assert!(result.contains("Final bill amount: 678.90"));
        assert!(result.contains("Discount given: 0.00"));
    }

    #[test]
    fn test_no_previous_week_data() {
        let mut conn = REDIS_CONNECTION.lock().unwrap();
        // Clear Redis before the test
        let _: () = redis::cmd("FLUSHALL").query(&mut *conn).unwrap();
        let keys_after_flush: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut *conn).unwrap();
        println!("Keys after FLUSHALL: {:?}", keys_after_flush);

        let phone = "9876543210";
        let business_name = "test102";
        let token = generate_and_store_token(phone, business_name, &mut *conn);

        // No previous week data
        // No current week data
        let result = get_response(
            token,
            business_name.to_string(),
            format!("{}, 678.90", phone),
            &mut *conn,
        );
        println!("Test no_previous_week_data: {}", result);
        // Expected: No discount since there’s no previous week data
        // Final bill: 678.90 - 0.0 = 678.90
        // Discount percentage: 0.0%
        assert!(result.contains("Final bill amount: 678.90"));
        assert!(result.contains("Discount given: 0.00"));
    }

    #[test]
    fn test_low_bill_amount_no_cap() {
        let mut conn = REDIS_CONNECTION.lock().unwrap();
        // Clear Redis before the test
        let _: () = redis::cmd("FLUSHALL").query(&mut *conn).unwrap();
        let keys_after_flush: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut *conn).unwrap();
        println!("Keys after FLUSHALL: {:?}", keys_after_flush);

        let phone = "9876543210";
        let business_name = "test102";
        let token = generate_and_store_token(phone, business_name, &mut *conn);

        // Setup previous week data
        setup_previous_week_data(&mut *conn, business_name, phone, 30.0, 1.0);

        // Explicitly set current week data with total_eligible_customers = 0.0
        setup_current_week_data(&mut *conn, business_name, "different_phone", false, 0.0);

        let result = get_response(
            token,
            business_name.to_string(),
            format!("{}, 100.00", phone),
            &mut *conn,
        );
        println!("Test low_bill_amount_no_cap: {}", result);
        // Expected discount: 30.0 / (1.0 + 0.0) = 30.0
        // Final bill: 100.00 - 30.0 = 70.00
        // Discount percentage: (30.0 / 100.00) * 100 = 30.0%
        assert!(result.contains("Final bill amount: 70.00"));
        assert!(result.contains("Discount given: 30.00"));
    }

    #[test]
    fn test_multiple_eligible_customers_current_week() {
        let mut conn = REDIS_CONNECTION.lock().unwrap();
        // Clear Redis before the test
        let _: () = redis::cmd("FLUSHALL").query(&mut *conn).unwrap();
        let keys_after_flush: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut *conn).unwrap();
        println!("Keys after FLUSHALL: {:?}", keys_after_flush);

        let phone = "9876543210";
        let business_name = "test102";
        let token = generate_and_store_token(phone, business_name, &mut *conn);

        // Setup previous week data
        setup_previous_week_data(&mut *conn, business_name, phone, 30.0, 1.0);

        // Setup current week data with other eligible customers (but no transaction for the test phone)
        setup_current_week_data(&mut *conn, business_name, "different_phone", false, 2.0);

        let result = get_response(
            token,
            business_name.to_string(),
            format!("{}, 678.90", phone),
            &mut *conn,
        );
        println!("Test multiple_eligible_customers_current_week: {}", result);
        // Expected discount: 30.0 / (1.0 + 2.0) = 30.0 / 3.0 = 10.0
        // Final bill: 678.90 - 10.0 = 668.90
        // Discount percentage: (10.0 / 678.90) * 100 ≈ 1.47%
        assert!(result.contains("Final bill amount: 668.90"));
        assert!(result.contains("Discount given: 1.47"));
    }

    #[test]
    fn test_get_response_no_token() {
        let mut conn = REDIS_CONNECTION.lock().unwrap();
        // Clear Redis before the test
        let _: () = redis::cmd("FLUSHALL").query(&mut *conn).unwrap();
        let keys_after_flush: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut *conn).unwrap();
        println!("Keys after FLUSHALL: {:?}", keys_after_flush);

        let result = get_response(
            "test_token_fail".to_string(),
            "test102".to_string(),
            "9876543210, 678.90".to_string(),
            &mut *conn,
        );
        println!("Test get_response_no_token: {}", result);
        assert_eq!(result, "Not authorized / Token expired.");
    }

    #[test]
    fn test_get_response_wrong_user() {
        let mut conn = REDIS_CONNECTION.lock().unwrap();
        // Clear Redis before the test
        let _: () = redis::cmd("FLUSHALL").query(&mut *conn).unwrap();
        let keys_after_flush: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut *conn).unwrap();
        println!("Keys after FLUSHALL: {:?}", keys_after_flush);

        let phone = "9876543210";
        let token = generate_and_store_token(phone, "test102", &mut *conn);
        let result = get_response(
            token,
            "test101".to_string(),
            "9876543210, 678.90".to_string(),
            &mut *conn,
        );
        println!("Test get_response_wrong_user: {}", result);
        assert_eq!(result, "Not authorized / Token expired.");
    }
}
