use std::{
    collections::HashMap,
    env,
    fs
};

use anyhow::{
    Context,
    Result
};

use aws_sdk_s3::{
    self,
    types::ByteStream
};

use lambda_runtime::{
    LambdaEvent,
    Error,
    service_fn
};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct DeviceProfilePublisherParameters {
    version: String
}

#[derive(Deserialize, Debug)]
enum RequestType {
    Create,
    Update,
    Delete
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct CloudFormationRequest {
    request_id: String,
    request_type: RequestType,
    #[serde(rename = "ResponseURL")]
    response_url: String,
    #[allow(dead_code)]
    resource_type: String,
    logical_resource_id: String,
    stack_id: String,
    #[allow(dead_code)]
    physical_resource_id: Option<String>,
    resource_properties: DeviceProfilePublisherParameters
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "UPPERCASE")]
enum ResponseType {
    Success,
    Failed
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct CloudFormationResponse {
    status: ResponseType,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    physical_resource_id: String,
    stack_id: String,
    request_id: String,
    logical_resource_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    no_echo: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<HashMap<String, String>>,
}

const MOBILE_CONFIG_FILENAME: &str = "dns.mobileconfig";

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(handler)).await?;

    Ok(())
}

async fn handler(event: LambdaEvent<CloudFormationRequest>) -> Result<(), Error> {
    let request = event.payload;
    
    match handle_request(&request).await {
        Ok(_) => send_cloudformation_success(&request, MOBILE_CONFIG_FILENAME).await,
        Err(err) => {
            println!("{:?}", err);
            send_cloudformation_failure(&request, MOBILE_CONFIG_FILENAME, &err.to_string()).await
        }
    };

    Ok(())
}

async fn handle_request(request: &CloudFormationRequest) -> Result<(), Error> {
    println!("Input event: {:#?}", request);

    let aws_config = aws_config::load_from_env().await;
    let s3_client = aws_sdk_s3::Client::new(&aws_config);
    let version = &request.resource_properties.version;

    match request.request_type {
        RequestType::Create => put_mobile_config(&s3_client, &version).await,
        RequestType::Update => put_mobile_config(&s3_client, &version).await,
        RequestType::Delete => delete_mobile_config(&s3_client).await
    }
}

async fn put_mobile_config(s3_client: &aws_sdk_s3::Client, version: &str) -> Result<(), Error> {
    println!("Uploading {} file...", MOBILE_CONFIG_FILENAME);

    let bucket_name = env::var("APPLE_DEVICE_PROFILE_BUCKET_NAME").with_context(|| "APPLE_DEVICE_PROFILE_BUCKET_NAME env var not set")?;
    let resolver_url = env::var("RESOLVER_URL").with_context(|| "RESOLVER_URL env var not set")?;

    let device_profile_contents = fs::read_to_string(MOBILE_CONFIG_FILENAME)
        .with_context(|| format!("Missing Apple device profile template file '{}'", MOBILE_CONFIG_FILENAME))?
        .replace("##RESOLVER_URL##", &resolver_url)
        .replace("##VERSION##", &version.to_string());

    s3_client
        .put_object()
        .bucket(bucket_name)
        .key(MOBILE_CONFIG_FILENAME)
        .content_type("application/x-apple-aspen-config")
        .body(ByteStream::from(device_profile_contents.as_bytes().to_vec()))
        .send()
        .await?;

    println!("Uploaded {} file", MOBILE_CONFIG_FILENAME);

    Ok(())
}

async fn delete_mobile_config(s3_client: &aws_sdk_s3::Client) -> Result<(), Error> {
    println!("Deleting {} file...", MOBILE_CONFIG_FILENAME);

    let bucket_name = env::var("APPLE_DEVICE_PROFILE_BUCKET_NAME")?;
    
    s3_client
        .delete_object()
        .bucket(bucket_name)
        .key("dns.mobileconfig")
        .send()
        .await?;

    println!("Deleted {} file", MOBILE_CONFIG_FILENAME);
    
    Ok(())
}

async fn send_cloudformation_success(request: &CloudFormationRequest, physical_resource_id: &str) {
    let response = CloudFormationResponse {
        status: ResponseType::Success,
        physical_resource_id: physical_resource_id.to_string(),
        stack_id: request.stack_id.to_owned(),
        request_id: request.request_id.to_owned(),
        logical_resource_id: request.logical_resource_id.to_owned(),
        reason: Option::None,
        no_echo: Option::None,
        data: Option::None
    };

    send_cloudformation_response(&request.response_url, &response).await;
}

async fn send_cloudformation_failure(request: &CloudFormationRequest, physical_resource_id: &str, reason: &str) {
    let response = CloudFormationResponse {
        status: ResponseType::Failed,
        physical_resource_id: physical_resource_id.to_string(),
        stack_id: request.stack_id.to_owned(),
        request_id: request.request_id.to_owned(),
        logical_resource_id: request.logical_resource_id.to_owned(),
        reason: Option::Some(reason.to_string()),
        no_echo: Option::None,
        data: Option::None
    };
    
    send_cloudformation_response(&request.response_url, &response).await;
}

async fn send_cloudformation_response(response_url: &str, response: &CloudFormationResponse) {
    let client = reqwest::Client::new();

    println!("Sending CloudFormation response: {}", serde_json::to_string(response).unwrap());
    
    client.put(response_url)
        .json(&response)
        .send()
        .await
        .expect("Failed to send CloudFormation response")
        .error_for_status()
        .expect("Failed to send CloudFormation response");
}