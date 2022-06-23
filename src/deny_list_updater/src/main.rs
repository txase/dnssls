use std::{
    env,
    io::{
        Cursor,
        Write
    }
};

use aws_sdk_lambda::{
    model::Architecture,
    types::Blob
};

use lambda_runtime::{
    Error,
    LambdaEvent,
    service_fn
};

use regex::Regex;

use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(handler)).await?;

    Ok(())
}

async fn handler(_: LambdaEvent<Value>) -> Result<(), Error> {
    let responder_function_name = env::var("RESPONDER_FUNCTION_NAME")
        .expect("RESPONDER_FUNCTION_NAME env var not available");

    let aws_config = aws_config::load_from_env().await;
    let lambda_client = aws_sdk_lambda::Client::new(&aws_config);

    let package_future = get_code_package(&responder_function_name, &lambda_client);
    let deny_list_future = get_deny_list();

    let package = package_future.await?;
    let deny_list = deny_list_future.await?;

    println!("Downloaded code and deny list");

    let deny_list = simplify_deny_list(deny_list)?;

    println!("Simplified deny list");

    let package = update_code_package(package, deny_list)?;

    println!("Finished writing zip to buffer");

    upload_new_code_package(&responder_function_name, &lambda_client, package).await?;

    println!("Finished uploading new code package");

    Ok(())
}

async fn get_code_package(responder_function_name: &str, lambda_client: &aws_sdk_lambda::client::Client) -> Result<Vec<u8>, Error> {
    let responder_function_config = lambda_client
        .get_function()
        .function_name(responder_function_name)
        .send()
        .await?;

    let responder_code_location = responder_function_config.code
        .expect("Missing responder function code config")
        .location
        .expect("Missing responder function code location");

    println!("Got code location");

    Ok(reqwest::get(responder_code_location).await?
        .bytes().await?
        .as_ref()
        .to_vec()
    )
}

async fn get_deny_list() -> Result<String, Error> {
    const DENY_LIST_URL: &str = "https://raw.githubusercontent.com/StevenBlack/hosts/master/hosts";

    let bytes = reqwest::get(DENY_LIST_URL).await?
        .bytes().await?;

    Ok(std::str::from_utf8(&bytes)?.to_string())
}

fn simplify_deny_list(deny_list: String) -> Result<String, Error> {
    let simplify_re = Regex::new(r"(?m)^0.0.0.0 .*$")?;

    let mut simplified = String::with_capacity(deny_list.len());

    for line in simplify_re.find_iter(&deny_list) {
        let domain = &deny_list[line.start() + 8..line.end()];

        if domain.ne("0.0.0.0") {
            simplified.push_str(domain);
            simplified.push('\n');
        }
    }

    simplified.pop();

    Ok(simplified)
}

fn update_code_package(package: Vec<u8>, deny_list: String) -> Result<Vec<u8>, Error> {
    let buffer = Cursor::new(package);

    let mut reader = zip::ZipArchive::new(buffer)?;

    let buffer = Cursor::new(Vec::new());

    let mut writer = zip::ZipWriter::new(buffer);

    writer.raw_copy_file(reader.by_name("bootstrap")?)?;

    writer.start_file("hosts", zip::write::FileOptions::default())?;

    writer.write_all(deny_list.as_bytes())?;

    Ok(writer.finish()?.into_inner())
}

async fn upload_new_code_package(responder_function_name: &str, lambda_client: &aws_sdk_lambda::client::Client, package: Vec<u8>) -> Result<(), Error> {
    lambda_client
        .update_function_code()
        .function_name(responder_function_name)
        .zip_file(Blob::new(package))
        .architectures(Architecture::Arm64)
        .send()
        .await?;

    Ok(())
}