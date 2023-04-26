use crate::auth::register::Device;
use crate::auth::Token;
use fireauth::FireAuth;
use reqwest::{RequestBuilder, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

pub(crate) trait RequestJson {
    fn set_token_id(&mut self, id_token: &str);
}

#[derive(Serialize, Debug, Deserialize)]
pub struct DeviceRegisterJson {
    pub(crate) id_token: String,
    pub(crate) device_name: String,
}

macro_rules! impl_request_json_for_structs {
    ($($struct_name:ident),*) => {
        $(
            impl RequestJson for $struct_name {
                fn set_token_id(&mut self, id_token: &str) {
                    self.id_token = id_token.to_string();
                }
            }
        )*
    };
}

impl_request_json_for_structs!(
    DeviceRegisterJson,
    GetSafeExitIdJson,
    CheckSafeExitIdJson,
    EventBodyJson
);

#[derive(Serialize, Debug, Deserialize)]
pub struct EventBodyJson {
    pub(crate) id_token: String,
    pub(crate) device_uuid: String,
    pub(crate) event: HashMap<String, i32>,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct GetSafeExitIdJson {
    pub(crate) id_token: String,
    pub(crate) device_uuid: String,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct CheckSafeExitIdJson {
    pub(crate) id_token: String,
    pub(crate) device_uuid: String,
    pub(crate) safe_exit_id: String,
}

/// Make a reqwest request, and refresh the id_token if necessary
pub(crate) async fn make_request_with_id_token<T: Serialize + ?Sized + RequestJson>(
    fire_auth: &FireAuth,
    device: &mut Device,
    req: RequestBuilder,
    json: &mut T,
) -> Result<reqwest::Response, Box<dyn Error>> {
    let mut original_req = req.try_clone().unwrap();
    let res = req.send().await?;
    match res.status() {
        StatusCode::UNAUTHORIZED => {
            device.id_token = refresh_id_token(device.refresh_token.clone(), fire_auth)
                .await
                .unwrap();

            json.set_token_id(&device.id_token);
            original_req = original_req.json(json);
            // Run the request again
            Ok(original_req.send().await?)
        }
        StatusCode::PAYMENT_REQUIRED => {
            // No active subscription for the user. Error out.
            Err("No active subscription".into())
        }
        _ => Ok(res),
    }
}

/// Refreshes the token. Will loop forever until successful.
pub(crate) async fn refresh_id_token(
    og_token: String,
    auth: &FireAuth,
) -> Result<Token, Box<dyn Error>> {
    Ok(auth.refresh_id_token(&og_token).await?.id_token)
}
