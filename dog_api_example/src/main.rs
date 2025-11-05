use serde::Deserialize;
use std::error::Error;

#[derive(Debug, Deserialize)]
struct DogImage {
    message: String,
    status: String,
}

#[derive(Debug)]
enum ApiResult {
    Success(DogImage),
    ApiError(String),
    NetworkError(String),
}

fn fetch_random_dog_image() -> ApiResult {
    let url = "https://dog.ceo/api/breeds/image/random";

    match ureq::get(url).call() {
        Ok(response) => {
            if response.status() == 200 {
                match response.into_json::<DogImage>() {
                    Ok(dog_image) => ApiResult::Success(dog_image),
                    Err(e) => ApiResult::ApiError(format!("Failed to parse JSON: {}", e)),
                }
            } else {
                ApiResult::ApiError(format!("HTTP error: {}", response.status()))
            }
        }
        Err(e) => {
            let error_details = format!("Request failed: {}", e);
            ApiResult::NetworkError(error_details)
        }
    }
}

enum DownloadError {
    Io(std::io::Error),
    Ureq,
}

impl From<std::io::Error> for DownloadError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ureq::Error> for DownloadError {
    fn from(_value: ureq::Error) -> Self {
        Self::Ureq
    }
}

fn download_image(url: &str, i: usize) -> Result<(), DownloadError> {
    let file = std::fs::File::create(format!("dogs/{i}.jpeg"))?;
    let resp = ureq::get(url).call()?;

    let mut reader = resp.into_reader();
    let mut writer = std::io::BufWriter::new(file);

    std::io::copy(&mut reader, &mut writer)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Dog Image Fetcher");
    println!("=================\n");

    for i in 1..=5 {
        println!("Fetching random dog image #{}", i);
        match fetch_random_dog_image() {
            ApiResult::Success(dog_image) => {
                let Ok(_) = download_image(dog_image.message.as_str(), i) else {
                    eprintln!("Failed to download image");
                    continue;
                };

                println!("✅ Success!");
                println!("🖼️ Image URL: {}", dog_image.message);
                println!("📊 Status: {}", dog_image.status);
            }
            ApiResult::ApiError(e) => println!("❌ API Error: {}", e),
            ApiResult::NetworkError(e) => println!("❌ Network Error: {}", e),
        }
        println!();
    }

    Ok(())
}
