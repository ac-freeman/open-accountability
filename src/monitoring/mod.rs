use crate::ImageReader;
//
pub const FLAG_PATH: &str = "./.flag";
//
pub(crate) async fn monitor(
    running: Arc<AtomicBool>,
    mut lt: LepTess,
    auth: &mut Auth,
) -> Result<(), OpenAccError> {
    let mut blacklist = get_blacklist(auth).await?; // TODO: Handle if no internet connection here

    while running.load(Ordering::SeqCst) {
        info!("starting loop");
        rotate_log()?;
        info!("rotated log");

        // Reset the blacklist values
        for v in blacklist.values_mut() {
            *v = 0;
        }

        let screens = match Screen::all() {
            Ok(screens) => {
                warn!("screens: {}", screens.len());
                screens
            }
            Err(e) => {
                warn!("Screen error");
                warn!("Error: {}", e);
                continue;
            }
        };

        for screen in screens {
            let start = Instant::now();
            let image = screen.capture().unwrap();
            let buffer = image.buffer();

            let img = ImageReader::new(Cursor::new(buffer))
                .with_guessed_format()?
                .decode()?;

            analyze_image(img, &mut lt, &mut blacklist, &running)?;

            warn!("elapsed time: {:?}", start.elapsed());
        }
        let mut event_map = HashMap::new();
        for (keyword, count) in blacklist.iter() {
            if *count > 0 {
                warn!("{}: count {}", keyword, count);
                event_map.insert(keyword.clone(), *count);
            }
        }

        post_event(auth, event_map).await?;

        let sleep_seconds = min(
            MAX_SLEEP_SECONDS,
            MIN_SLEEP_SECONDS + rand::random::<u64>() % (MAX_SLEEP_SECONDS - MIN_SLEEP_SECONDS),
        );

        // Loop for thread sleep to prevent busy-waiting and allow detection of SIGTERM
        for _ in 0..sleep_seconds {
            if !running.load(Ordering::SeqCst) {
                info!("Exiting due to SIGTERM");

                return Err(OpenAccError::SigTerm);
            }
            thread::sleep(time::Duration::from_secs(1));
        }
    }
    Ok(())
}

use crate::auth::Auth;
use crate::{
    OpenAccError, LOG_FILE_LINE_COUNT_LIMIT, LOG_PATH, MAX_SLEEP_SECONDS, MIN_SLEEP_SECONDS,
};
use image::imageops::crop_imm;
use image::DynamicImage;
use leptess::LepTess;
use screenshots::Screen;
use serde_json::Value;

use crate::requests::{make_request_with_id_token, EventBodyJson};

use std::cmp::min;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::{fs, thread, time};

fn rotate_log() -> Result<(), Box<dyn Error>> {
    let lines = BufReader::new(File::open(LOG_PATH)?).lines();
    if lines.count() > LOG_FILE_LINE_COUNT_LIMIT {
        let lines_text = BufReader::new(File::open(LOG_PATH)?)
            .lines()
            .skip(LOG_FILE_LINE_COUNT_LIMIT / 2)
            .map(|x| x.unwrap())
            .collect::<Vec<String>>()
            .join("\n");

        fs::write(LOG_PATH, lines_text)?;
    }
    Ok(())
}

async fn get_blacklist(auth: &mut Auth) -> Result<HashMap<String, i32>, Box<dyn Error>> {
    let res = auth
        .client
        .get("https://us-central1-openaccountability.cloudfunctions.net/getBlacklist")
        .bearer_auth(auth.device.refresh_token.clone())
        .send()
        .await?;

    let json_body: Value = res.json().await?;

    // Flatten the arrays of "keywords_high", "keywords_mid", and "keywords_low" into a vec
    let mut blacklist_vec: Vec<String> = Vec::new();
    for key in ["keywords_high", "keywords_mid", "keywords_low"].iter() {
        let keywords = json_body[key].as_array().unwrap();
        for keyword in keywords {
            blacklist_vec.push(keyword.as_str().unwrap().to_string());
        }
    }

    // Turn blacklist vec into a HashMap for faster lookup
    let mut blacklist: HashMap<String, i32> = HashMap::new();
    for item in blacklist_vec.iter() {
        if !blacklist.contains_key(item) {
            blacklist.insert(item.clone(), 0);
        }
    }

    Ok(blacklist)
}

fn analyze_image(
    img: DynamicImage,
    lt: &mut leptess::LepTess,
    blacklist: &mut HashMap<String, i32>,
    running: &Arc<AtomicBool>,
) -> Result<(), OpenAccError> {
    let slice_height = 512;
    let mut tiff_buffer = Vec::new();
    for i in (0..img.height()).step_by(slice_height) {
        info!("slice {} - {}", i, i + slice_height as u32);
        let start_slice = Instant::now();
        tiff_buffer.clear(); // Remove the data, but keep the allocated memory
        let crop = crop_imm(&img, 0, i, img.width(), slice_height as u32);

        // Convert to tiff so that tesseract/leptonica can read it on Windows
        crop.to_image().write_to(
            &mut Cursor::new(&mut tiff_buffer),
            image::ImageOutputFormat::Tiff,
        )?;
        lt.set_image_from_mem(&tiff_buffer).unwrap();
        lt.set_source_resolution(100);

        lt.get_utf8_text()?
            .to_lowercase() // Blacklist is all in lowercase
            // Remove punctuation. Will help blacklist website URLs in the future
            .replace(&['(', ')', ',', '\"', '.', ';', ':', '\''][..], " ")
            .split_whitespace() // Split into words
            .for_each(|x|
                // If string is in the blacklist, then increment its count
                if blacklist.contains_key(x) {
                    blacklist.insert(x.to_string(), 1 + blacklist[x]);

                    // Get the position of the item using the index_map
                    // let index = blacklist.get(x).unwrap();
                    println!("blacklist {} now has count {}", x, blacklist[x]);
                    // blacklist_counts[*index] += 1;
                }
            );
        let time_elapsed = start_slice.elapsed();
        info!("slice time: {:?}", start_slice.elapsed());

        let sleep_seconds = time_elapsed.as_secs() * 10;

        for _ in 0..sleep_seconds {
            if !running.load(Ordering::SeqCst) {
                info!("Exiting due to SIGTERM");
                return Err(OpenAccError::SigTerm);
            }
            thread::sleep(time::Duration::from_secs(1));
        }
    }
    Ok(())
}

async fn post_event(
    auth: &mut Auth,
    event_map: HashMap<String, i32>,
) -> Result<(), Box<dyn Error>> {
    info!("about to post");
    let mut request_json = EventBodyJson {
        id_token: auth.device.id_token.clone(),
        device_uuid: auth.device.uuid.clone(),
        event: event_map,
    };
    let request_builder = auth
        .client
        .post("https://us-central1-openaccountability.cloudfunctions.net/api/event")
        .bearer_auth(auth.device.refresh_token.clone())
        .json(&request_json);
    let res = make_request_with_id_token(
        &auth.fire_auth,
        &mut auth.device,
        request_builder,
        &mut request_json,
    )
    .await?;

    let response_text = res.text().await?;
    info!("response: {}", response_text);
    info!("done posting");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::auth::register::Device;
    use pretty_assertions::assert_eq;
    use reqwest::Client;
    use rocket::futures::TryFutureExt;

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
    async fn test_post_event() {
        create_device_info_file_from_env();

        let client = Client::new();
        let mut auth = Auth::new(client).await.unwrap();
        assert!(!auth.device.safe_shutdown_id.is_empty());
        assert_ne!(auth.device.uuid, dotenv!("TEST_DEVICE_UID").to_string());
        assert_eq!(
            auth.device.refresh_token,
            dotenv!("TEST_REFRESH_TOKEN").to_string()
        );

        let mut event_map = HashMap::new();
        event_map.insert("testkeyword1".to_string(), 5);

        post_event(&mut auth, event_map).await.unwrap();
    }

    /// Get the blacklist from the server
    #[tokio::test]
    async fn test_get_blacklist() {
        create_device_info_file_from_env();

        let client = Client::new();
        let mut auth = Auth::new(client).await.unwrap();
        let blacklist = get_blacklist(&mut auth).await.unwrap();
        assert!(blacklist.len() > 20);
    }

    #[tokio::test]
    async fn test_analyze_images() {
        let mut lt = leptess::LepTess::new(None, "eng").unwrap();
        let running = Arc::new(AtomicBool::new(true));

        {
            let img = ImageReader::open("tests/test_image.png")
                .unwrap()
                .with_guessed_format()
                .unwrap()
                .decode()
                .unwrap();

            let mut blacklist: HashMap<String, i32> = HashMap::new();
            blacklist.insert("testkeyword1".to_string(), 0);

            analyze_image(img, &mut lt, &mut blacklist, &running).unwrap();
            // Check that we detected a reasonable number of the test keyword
            assert!(blacklist["testkeyword1"] > 10);
        }

        {
            let img = ImageReader::open("tests/test_image2.png")
                .unwrap()
                .with_guessed_format()
                .unwrap()
                .decode()
                .unwrap();

            let mut blacklist: HashMap<String, i32> = HashMap::new();
            blacklist.insert("testkeyword1".to_string(), 0);

            analyze_image(img, &mut lt, &mut blacklist, &running).unwrap();
            // Check that we detected a reasonable number of the test keyword
            eprintln!("count: {}", blacklist["testkeyword1"]);
            assert!(blacklist["testkeyword1"] > 7);
            // TODO: Figure out how to improve performance on higher-res images
        }
    }
}
