#[macro_use]
extern crate lazy_static;

use std::{
    borrow::Cow,
    collections::HashSet,
    fmt,
    fs::File,
    io::{self, BufRead},
    path::Path
};

// Enable arbitrary error bubbling
use anyhow::Result;

use lambda_http::{
    http::Method,
    request::RequestContext::{
        ApiGatewayV1,
        ApiGatewayV2
    },
    service_fn,
    Request,
    RequestExt,
    Response,
    Body,
    http::StatusCode
};

use trust_dns_proto::{
    op::{
        header::MessageType,
        message::Message,
        response_code::ResponseCode::NXDomain
    },
    serialize::binary::{
        BinDecodable,
        BinEncodable
    },
    xfer::DnsRequestOptions
};

use trust_dns_resolver::{
    error::ResolveErrorKind::{
        NoRecordsFound,
        Proto
    },
    TokioAsyncResolver
};

use url::Url;

#[derive(Debug, Clone)]
struct BadRequestError {
    message: String
}

impl BadRequestError {
    pub fn new(message: &str) -> Self {
        Self { message: message.to_string() }
    }

    pub fn message(&self) -> String {
        self.message.clone()
    }
}

impl std::error::Error for BadRequestError {}

impl fmt::Display for BadRequestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "bad request: {}", self.message)
    }
}

lazy_static! {
    static ref HOSTS: HashSet<String> = {
        let mut hosts: HashSet<String> = HashSet::new();

        if let Ok(lines) = read_lines("./hosts") {
            // Consumes the iterator, returns an (Optional) String
            for line in lines {
                if let Ok(host) = line {
                    hosts.insert(host);
                }
            }
        }

        hosts
    };

    static ref RESOLVER: TokioAsyncResolver = {
        TokioAsyncResolver::tokio_from_system_conf().expect("Failed to create async resolver")
    };
}

#[tokio::main]
async fn main() -> Result<(), lambda_http::Error> {
    lambda_http::run(service_fn(respond)).await?;

    Ok(())
}

// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

async fn respond(request: Request) -> Result<Response<Body>, lambda_http::Error> {
    let ip = match request.request_context() {
        ApiGatewayV1(context) => context.identity.source_ip.unwrap_or("Unknown".to_string()),
        ApiGatewayV2(context) => context.http.source_ip.unwrap_or("Unknown".to_string()),
        _ => "Unknown".to_string()
    };

    println!("Received request from Client IP: {}", ip);

    let message = match *request.method() {
        Method::GET => message_from_get(request).await,
        Method::POST => message_from_post(request).await,
        _ => return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::from(()))?)
    };

    let message = match message {
        Ok(message) => message,
        Err(err) => {
            return match err.downcast_ref::<BadRequestError>() {
                Some(err) => {
                    println!("Bad request: {}", err.message());
                    Ok(Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from(format!("Bad request: {}", err.message())))?)
                },
                None => {
                    println!("Failed to process request: {}", err);
                    return Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from(()))?);
                }
            };
        }
    };

    // While the DNS protocol supports multiple questions in theory,
    // in practice no one supports it (i.e. BIND doesn't...)
    let query = &message.queries()[0];
    let domain = query.name().to_utf8();
    let mut domain_without_last_period = domain.clone();

    if domain.len() > 0 && domain.chars().last().unwrap() == '.' {
        domain_without_last_period.remove(domain.chars().count() - 1);
    }

    let mut response = message.clone();
    response
        .set_message_type(MessageType::Response)
        .set_recursion_available(true);

    if HOSTS.contains(&domain_without_last_period) {
        println!("Domain '{}' matches denylist, returning NXDomain", domain);
        response.set_response_code(NXDomain);
    } else {
        println!("Domain '{}' does not match denylist, proxying query...", domain);
        let results = RESOLVER
            .lookup(domain, query.query_type(), DnsRequestOptions::default())
            .await;

        match results {
            Ok(results) => {
                for answer in results.record_iter() {
                    response.add_answer(answer.clone());
                }
            },
            Err(err) => {
                match err.kind() {
                    NoRecordsFound { .. } => {
                        response.set_response_code(NXDomain);
                    },
                    Proto(_) => {
                        println!("Invalid domain: {}", domain_without_last_period);
                        response.set_response_code(NXDomain);
                    },
                    _ => {
                        println!("Failed to query for domain: {}", err);
                        return Ok(Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Body::from(()))?);
                    }
                };
            }
        };
    };

    let response_bytes = response.to_bytes().expect("Failed to serialize response");

    println!("Done!");

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/dns-message")
        .body(Body::from(response_bytes))?)
}

async fn message_from_get(request: Request) -> Result<Message> {
    println!("URI: {}", request.uri().to_string());

    let url = Url::parse(&request.uri().to_string())?;

    let encoded_payload = match url.query_pairs().find(|pair| pair.0 == Cow::Borrowed("dns")) {
        Some(pair) => pair.1,
        None => return Err(BadRequestError::new("Missing 'dns' query string parameter"))?
    };

    let payload = match base64_url::decode(&encoded_payload.to_string()) {
        Ok(payload) => payload,
        Err(err) => {
            println!("Failed to base64 decode DNS message '{}': {}", encoded_payload, err);
            return Err(BadRequestError::new("Invalid DNS message"))?;
        }
    };

    match Message::from_bytes(payload.as_ref()) {
        Ok(message) => Ok(message),
        Err(err) => {
            println!("Failed to parse DNS message: {}", err);
            Err(BadRequestError::new("Invalid DNS message"))?
        }
    }
}

async fn message_from_post(request: Request) -> Result<Message> {
    let body = request.body();

    match body {
        Body::Empty => Err(BadRequestError::new("Empty body"))?,

        Body::Text(_) => Err(BadRequestError::new("Text body"))?,

        Body::Binary(data) => match Message::from_bytes(data.as_ref()) {
            Ok(message) => {
                println!("dns request message base64-URL encoded: {}", base64_url::encode(data));
                Ok(message)
            },
            Err(err) => {
                println!("Failed to parse DNS message: {}", err);
                Err(BadRequestError::new("Invalid DNS message"))?
            }
        }
    }
}