use crate::auth::{Token, DEVICE_INFO_PATH};
use crate::requests::{
    make_request_with_id_token, refresh_id_token, CheckSafeExitIdJson, DeviceRegisterJson,
    GetSafeExitIdJson,
};
use fireauth::FireAuth;
use std::borrow::Cow;

use reqwest::{Client, Response, StatusCode};
use rocket::http::ContentType;
use rocket::response::content::RawHtml;
use rocket::serde::json::Json;
use rocket::{Build, Rocket, State};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Debug, Deserialize)]
struct WebLoginCredentials {
    refresh_token: String,
    device_name: String,
}

struct ChannelState {
    tx: Arc<Mutex<Sender<WebLoginCredentials>>>,
}

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "src/auth/static/"]
struct Images;

#[post("/login", format = "json", data = "<user_credentials>")]
fn login(user_credentials: Json<WebLoginCredentials>, tx: &State<ChannelState>) -> String {
    info!("IN LOGIN");
    info!("User credentials: {:?}", user_credentials);
    // let tmp = tx.tx.lock().unwrap().se;
    tx.tx
        .lock()
        .unwrap()
        .send(WebLoginCredentials {
            refresh_token: user_credentials.refresh_token.clone(),
            device_name: user_credentials.device_name.clone(),
        })
        .expect("Unable to send on channel");
    info!("attempted to send on channel");

    format!("Welcome, {}!", user_credentials.refresh_token)
}

#[get("/")]
async fn index() -> RawHtml<String> {
    RawHtml(String::from(include_str!("index.html")))
}

#[get("/static/<file..>")]
fn image(file: PathBuf) -> Option<(ContentType, Cow<'static, [u8]>)> {
    let filename = file.display().to_string();
    let asset = Images::get(&filename)?;
    let content_type = file
        .extension()
        .and_then(OsStr::to_str)
        .and_then(ContentType::from_extension)
        .unwrap_or(ContentType::Bytes);

    Some((content_type, asset.data))
}

fn rocket() -> Rocket<Build> {
    rocket::build().mount("/", routes![index, login, image])
}

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct Device {
    pub refresh_token: Token,
    pub id_token: Token,
    pub uuid: String,
    pub(crate) name: String,
    pub safe_shutdown_id: String,
}

impl Device {
    pub async fn new(fire_auth: &FireAuth, client: &Client) -> Result<Device, Box<dyn Error>> {
        let mut device = Device {
            refresh_token: String::new(),
            id_token: String::new(),
            uuid: String::new(),
            name: String::new(),
            safe_shutdown_id: "".to_string(),
        };

        device.webpage_sign_in().await?;
        device.id_token = refresh_id_token(device.refresh_token.clone(), fire_auth).await?;
        match device.register_device(fire_auth, client).await {
            Ok(res) if res.status() == StatusCode::OK => {
                info!("Successfully registered device");
                device.uuid = res.text().await?;
            }
            a => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to register device: {:?}", a),
                )));
            }
        }

        Ok(device)
    }

    /// Loads the device info from the local file and refreshes the id token.
    pub async fn from_file(auth: &FireAuth) -> Result<Device, Box<dyn Error>> {
        let contents = fs::read_to_string(DEVICE_INFO_PATH)?;

        let mut device: Device = serde_json::from_str(&contents)?;
        device.id_token = refresh_id_token(device.refresh_token.clone(), auth).await?;
        Ok(device)
    }

    /// Launches a local server and opens the web page for user to type in username and password.
    ///
    /// Errors if any of those processes fail.
    async fn webpage_sign_in(&mut self) -> Result<(), Box<dyn Error>> {
        Command::new("xdg-open")
            .arg("http://localhost:8000")
            .spawn()?;

        let (tx, rx) = channel::<WebLoginCredentials>();
        let rocket = rocket()
            .manage(ChannelState {
                tx: Arc::new(Mutex::new(tx)),
            })
            .ignite()
            .await?;
        let shutdown_handle = rocket.shutdown();

        rocket::tokio::spawn(rocket.launch());

        // Block until the user logs in on web browser
        let web_login_credentials = rx.recv()?;
        self.refresh_token = web_login_credentials.refresh_token;
        self.name = web_login_credentials.device_name;

        shutdown_handle.notify();

        Ok(())
    }

    pub async fn register_device(
        &mut self,
        fire_auth: &FireAuth,
        client: &Client,
    ) -> Result<Response, Box<dyn Error>> {
        let mut request_json = DeviceRegisterJson {
            id_token: self.id_token.clone(),
            device_name: self.name.clone(),
        };
        let request_builder = client
            .post("https://us-central1-openaccountability.cloudfunctions.net/api/device")
            .bearer_auth(self.refresh_token.as_str())
            .json(&request_json);
        let res =
            make_request_with_id_token(fire_auth, self, request_builder, &mut request_json).await?;

        Ok(res)
    }

    pub(crate) fn write_device_info(&self, with_safe_exit_id: bool) -> Result<(), Box<dyn Error>> {
        let serialized = if with_safe_exit_id {
            serde_json::to_string(&self)?
        } else {
            let mut device_tmp = self.clone();
            device_tmp.safe_shutdown_id = "".to_string();
            serde_json::to_string(&device_tmp)?
        };

        // Write the serialized data to a file
        fs::write(DEVICE_INFO_PATH, serialized)?;
        Ok(())
    }

    pub async fn get_safe_exit_id(
        &mut self,
        fire_auth: &FireAuth,
        client: &Client,
    ) -> Result<(), Box<dyn Error>> {
        let mut request_json = GetSafeExitIdJson {
            id_token: self.id_token.clone(),
            device_uuid: self.uuid.clone(),
        };
        let request_builder = client
            .post(
                "https://us-central1-openaccountability.cloudfunctions.net/api/device/safe_exit_id",
            )
            .bearer_auth(self.refresh_token.as_str())
            .json(&request_json);
        let res =
            make_request_with_id_token(fire_auth, self, request_builder, &mut request_json).await?;

        self.safe_shutdown_id = res.text().await?;
        Ok(())
    }

    pub async fn check_safe_exit_id(
        &mut self,
        fire_auth: &FireAuth,
        client: &Client,
    ) -> Result<(), Box<dyn Error>> {
        info!("Checking safe exit id");

        let mut request_json = CheckSafeExitIdJson {
            id_token: self.id_token.clone(),
            device_uuid: self.uuid.clone(),
            safe_exit_id: self.safe_shutdown_id.clone(),
        };
        let request_builder = client
            .patch(
                "https://us-central1-openaccountability.cloudfunctions.net/api/device/safe_exit_id",
            )
            .bearer_auth(self.refresh_token.as_str())
            .json(&request_json);
        let res =
            make_request_with_id_token(fire_auth, self, request_builder, &mut request_json).await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Safe exit id does not match",
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    fn create_device_info_file_from_env() {
        let device = Device {
            refresh_token: dotenv!("TEST_REFRESH_TOKEN").to_string(),
            id_token: "".to_string(),
            uuid: dotenv!("TEST_DEVICE_UID").to_string(),
            name: "".to_string(),
            safe_shutdown_id: "".to_string(),
        };
        device.write_device_info(false).unwrap();
    }

    #[tokio::test]
    async fn test_refresh_id_token() {
        let api_key: String = dotenv!("API_KEY").to_string();
        let refresh_token: String = dotenv!("TEST_REFRESH_TOKEN").to_string();
        let auth = fireauth::FireAuth::new(api_key);

        let id_token = refresh_id_token(refresh_token, &auth).await.unwrap();

        assert_ne!(id_token, ""); // TODO: Submit a PR to rust lang to add an error message to suggest assert_ne if user types assert_neq
    }

    #[tokio::test]
    async fn test_from_file() {
        create_device_info_file_from_env();

        let api_key: String = dotenv!("API_KEY").to_string();
        let auth = fireauth::FireAuth::new(api_key);

        let device = Device::from_file(&auth).await.unwrap();
        assert_ne!(device.id_token, "");
        assert_eq!(device.uuid, dotenv!("TEST_DEVICE_UID").to_string());
        assert_eq!(
            device.refresh_token,
            dotenv!("TEST_REFRESH_TOKEN").to_string()
        );
    }

    #[tokio::test]
    async fn test_register_device() {
        let client = Client::new();
        create_device_info_file_from_env();

        let api_key: String = dotenv!("API_KEY").to_string();
        let auth = fireauth::FireAuth::new(api_key);

        let mut device = Device::from_file(&auth).await.unwrap();
        let res = device.register_device(&auth, &client).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let uuid = res.text().await.unwrap();
        assert_ne!(uuid, device.uuid);
        assert_ne!(uuid, "");
    }

    #[tokio::test]
    async fn test_get_safe_exit_id() {
        let client = Client::new();
        create_device_info_file_from_env();

        let api_key: String = dotenv!("API_KEY").to_string();
        let auth = fireauth::FireAuth::new(api_key);

        let mut device = Device::from_file(&auth).await.unwrap();
        let res = device.register_device(&auth, &client).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let uuid = res.text().await.unwrap();
        assert_ne!(uuid.clone(), "");
        device.uuid = uuid;

        device.get_safe_exit_id(&auth, &client).await.unwrap();

        assert_eq!(device.safe_shutdown_id, "testexitid123".to_string());

        device.check_safe_exit_id(&auth, &client).await.unwrap();
    }

    #[tokio::test]
    async fn test_check_safe_exit_id() {
        let client = Client::new();
        create_device_info_file_from_env();

        let api_key: String = dotenv!("API_KEY").to_string();
        let auth = fireauth::FireAuth::new(api_key);

        let mut device = Device::from_file(&auth).await.unwrap();
        let res = device.register_device(&auth, &client).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let uuid = res.text().await.unwrap();
        assert_ne!(uuid.clone(), "");
        device.uuid = uuid;

        assert!(device.check_safe_exit_id(&auth, &client).await.is_err());
    }
}
