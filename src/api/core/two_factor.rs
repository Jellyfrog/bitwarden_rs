use rocket::Route;
use rocket::response::status::BadRequest;

use rocket_contrib::{Json, Value};

use data_encoding::BASE32;

use db::DbConn;
use db::models::*;

use util;
use crypto;

use auth::Headers;


#[get("/two-factor")]
fn get_twofactor(headers: Headers) -> Result<Json, BadRequest<Json>> {
    let data = if headers.user.totp_secret.is_none() {
        Value::Null
    } else {
        json!([{
            "Enabled": true,
            "Type": 0,
            "Object": "twoFactorProvider"
        }])
    };

    Ok(Json(json!({
        "Data": data,
        "Object": "list"
    })))
}

#[post("/two-factor/get-recover", data = "<data>")]
fn get_recover(data: Json<Value>, headers: Headers) -> Result<Json, BadRequest<Json>> {
    let password_hash = data["masterPasswordHash"].as_str().unwrap();

    if !headers.user.check_valid_password(password_hash) {
        err!("Invalid password");
    }

    Ok(Json(json!({
        "Code": headers.user.totp_recover,
        "Object": "twoFactorRecover"
    })))
}

#[post("/two-factor/get-authenticator", data = "<data>")]
fn generate_authenticator(data: Json<Value>, headers: Headers) -> Result<Json, BadRequest<Json>> {
    let password_hash = data["masterPasswordHash"].as_str().unwrap();

    if !headers.user.check_valid_password(password_hash) {
        err!("Invalid password");
    }

    let (enabled, key) = match headers.user.totp_secret {
        Some(secret) => (true, secret),
        _ => (false, BASE32.encode(&crypto::get_random(vec![0u8; 20])))
    };

    Ok(Json(json!({
        "Enabled": enabled,
        "Key": key,
        "Object": "twoFactorAuthenticator"
    })))
}

#[post("/two-factor/authenticator", data = "<data>")]
fn activate_authenticator(data: Json<Value>, headers: Headers, conn: DbConn) -> Result<Json, BadRequest<Json>> {
    let password_hash = data["masterPasswordHash"].as_str().unwrap();

    if !headers.user.check_valid_password(password_hash) {
        err!("Invalid password");
    }
    let token = data["token"].as_str(); // 123456
    let key = data["key"].as_str().unwrap(); // YI4SKBIXG32LOA6VFKH2NI25VU3E4QML

    // Validate key as base32 and 20 bytes length
    let decoded_key: Vec<u8> = match BASE32.decode(key.as_bytes()) {
        Ok(decoded) => decoded,
        _ => err!("Invalid totp secret")
    };

    if decoded_key.len() != 20 {
        err!("Invalid key length")
    }

    // Set key in user.totp_secret
    let mut user = headers.user;
    user.totp_secret = Some(key.to_uppercase());

    // Validate the token provided with the key
    if !user.check_totp_code(util::parse_option_string(token)) {
        err!("Invalid totp code")
    }

    // Generate totp_recover
    let totp_recover = BASE32.encode(&crypto::get_random(vec![0u8; 20]));
    user.totp_recover = Some(totp_recover);

    user.save(&conn);

    Ok(Json(json!({
        "Enabled": true,
        "Key": key,
        "Object": "twoFactorAuthenticator"
    })))
}

#[post("/two-factor/disable", data = "<data>")]
fn disable_authenticator(data: Json<Value>, headers: Headers, conn: DbConn) -> Result<Json, BadRequest<Json>> {
    let _type = &data["type"];
    let password_hash = data["masterPasswordHash"].as_str().unwrap();

    if !headers.user.check_valid_password(password_hash) {
        err!("Invalid password");
    }

    let mut user = headers.user;
    user.totp_secret = None;
    user.totp_recover = None;

    user.save(&conn);

    Ok(Json(json!({
        "Enabled": false,
        "Type": 0,
        "Object": "twoFactorProvider"
    })))
}
