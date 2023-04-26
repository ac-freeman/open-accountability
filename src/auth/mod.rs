pub mod register;

use crate::auth::register::Device;

use fireauth::FireAuth;
use log::info;
use reqwest::{Client, StatusCode};

use crate::SERVICE_NAME;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::Path;

#[cfg(not(test))]
pub const DEVICE_INFO_PATH: &str = "./.device";
#[cfg(test)]
pub const DEVICE_INFO_PATH: &str = "./tests/.device";

pub type Token = String;

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct TokenReqBody {
    pub(crate) id_token: String,
    pub(crate) device_id: String,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct TokenFile {
    refresh_token: String,
    device_id: String,
}

pub(crate) struct Auth {
    pub(crate) fire_auth: FireAuth,
    pub(crate) client: Client,
    pub(crate) device: Device,
}

impl Auth {
    /// Creates a new authenticated session. Does not return until successful authentication is
    /// achieved.
    pub async fn new(_client: Client) -> Result<Self, Box<dyn Error>> {
        let api_key: String = dotenv!("API_KEY").to_string();
        let fire_auth = fireauth::FireAuth::new(api_key);
        let client = Client::new();

        let mut device;
        if Path::new(DEVICE_INFO_PATH).exists() {
            device = Device::from_file(&fire_auth).await?;

            // Check the safe_exit_id
            if device
                .check_safe_exit_id(&fire_auth, &client)
                .await
                .is_err()
            {
                eprintln!("safe exit id is error");
                match device.register_device(&fire_auth, &client).await {
                    Ok(res) if res.status() == StatusCode::OK => {
                        info!("Successfully registered device");
                        device.uuid = res.text().await?;
                    }
                    a => {
                        dbg!(a).unwrap();
                    }
                }
            }
        } else {
            device = Device::new(&fire_auth, &client).await?;
        }

        device.get_safe_exit_id(&fire_auth, &client).await?;

        device.write_device_info(false)?;

        Ok(Self {
            fire_auth,
            client,
            device,
        })
    }

    pub(crate) async fn exit_program(
        &self,
        shutdown_in_progress: bool,
    ) -> Result<(), Box<dyn Error>> {
        if !shutdown_in_progress {
            info!("System is not in shut-down procedure. Posting exit event to server.");
            self.client
                .patch("https://us-central1-openaccountability.cloudfunctions.net/api/device")
                .bearer_auth(self.device.refresh_token.clone())
                .json(&TokenReqBody {
                    id_token: self.device.id_token.clone(),
                    device_id: self.device.uuid.clone(),
                })
                .send()
                .await?;
            fs::remove_file(DEVICE_INFO_PATH)?;
        } else {
            self.device.write_device_info(true)?;
        }
        info!("Exiting program");
        Ok(())
    }

    pub(crate) async fn check_service_file(&self) -> Result<(), Box<dyn Error>> {
        // Check that the .service file hasn't been modified
        let service_file_path = "/etc/systemd/system/".to_string() + SERVICE_NAME + ".service";
        // Read the file into a string
        let mut service_file = std::fs::File::open(service_file_path)?;
        let mut service_file_contents = String::new();
        service_file.read_to_string(&mut service_file_contents)?;
        eprintln!("{}", service_file_contents);
        // Ensure that the Restart line is correct
        // Ensure that "Restart" appears exactly once

        let mut restart = false;
        let mut restart_sec = false;
        for line in service_file_contents.lines() {
            match line {
                "Restart=always" => {
                    restart = true;
                }
                l if l.contains("Restart=") => {
                    restart = false;
                }
                "RestartSec=30s" => {
                    restart_sec = true;
                }
                l if l.contains("RestartSec=") => {
                    restart_sec = false;
                }
                _ => {}
            }
        }

        if !(restart && restart_sec) {
            // The service file has been tampered with! Terminate the device
            let _ = self.exit_program(false).await;
            return Err(Box::try_from("Service file has been tampered with").unwrap());
        }
        Ok(())
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

    /// Test re-registering a device
    #[tokio::test]
    async fn test_new_auth() {
        create_device_info_file_from_env();
        let client = Client::new();
        let auth = Auth::new(client).await.unwrap();
        assert!(!auth.device.safe_shutdown_id.is_empty());
        assert_ne!(auth.device.uuid, dotenv!("TEST_DEVICE_UID").to_string());
        assert_eq!(
            auth.device.refresh_token,
            dotenv!("TEST_REFRESH_TOKEN").to_string()
        );
        // assert_eq!(auth.state, AuthState::Authenticated("".to_string()));
    }

    #[tokio::test]
    async fn test_exit_program_safe() {
        create_device_info_file_from_env();
        let client = Client::new();
        let auth = Auth::new(client).await.unwrap();
        assert!(!auth.device.safe_shutdown_id.is_empty());
        assert_ne!(auth.device.uuid, dotenv!("TEST_DEVICE_UID").to_string());
        assert_eq!(
            auth.device.refresh_token,
            dotenv!("TEST_REFRESH_TOKEN").to_string()
        );

        let device_copy = auth.device.clone();

        auth.exit_program(true).await.unwrap();
        let contents = fs::read_to_string(DEVICE_INFO_PATH).unwrap();

        let device: Device = serde_json::from_str(&contents).unwrap();

        assert_eq!(
            device.safe_shutdown_id,
            device_copy.safe_shutdown_id.clone()
        );

        // Re-authenticate and make sure the device id is the same
        let client = Client::new();
        let auth = Auth::new(client).await.unwrap();
        assert_eq!(auth.device.uuid, device_copy.uuid.clone());

        // Below isn't true for this test, since the shutdown id is hardcoded for the test scenario
        // assert_ne!(auth.device.safe_shutdown_id, device_copy.safe_shutdown_id);
    }

    #[tokio::test]
    async fn test_exit_program_unsafe() {
        create_device_info_file_from_env();
        let client = Client::new();
        let auth = Auth::new(client).await.unwrap();
        assert!(!auth.device.safe_shutdown_id.is_empty());
        assert_ne!(auth.device.uuid, dotenv!("TEST_DEVICE_UID").to_string());
        assert_eq!(
            auth.device.refresh_token,
            dotenv!("TEST_REFRESH_TOKEN").to_string()
        );

        let _exit_id_copy = auth.device.safe_shutdown_id.clone();

        auth.exit_program(false).await.unwrap();

        // Check that the device file is deleted
        assert!(fs::metadata(DEVICE_INFO_PATH).is_err());
    }
}
