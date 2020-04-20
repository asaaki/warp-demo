//! Demonstration of using task local and the wrapping of warp filters as services for state data over full request-respone cycle.
//! Here we play with a RequestId data structure, which gets initialized conditionally and passed around, so it can be used in headers and body data.
//!
//! Motivation came from this issue and the discussion: https://github.com/seanmonstar/warp/issues/134
//! Code is based upon https://github.com/seanmonstar/warp/blob/master/examples/rejections.rs
//! and this snippet https://github.com/seanmonstar/warp/pull/408#issuecomment-578157715
//!
//! Provides `/math/<u16>` route and requires a `div-by: <u16>` header for doing the calculation.
//! Implements a "division by zero" error case, but also handles a few others.
//!
//! Example:
//! ```sh
//! curl -i http://localhost:3030/math/4 -H 'div-by: 2'
//! ```
//!
//! and should return a response like:
//! ```txt
//! HTTP/1.1 200 OK
//! content-type: application/json
//! x-request-id: internal-87ca5d23-7d18-4485-b0c1-bff48a67a9a4
//! content-length: 231
//! date: Mon, 20 Apr 2020 14:32:29 GMT
//!
//! {
//!   "op": "4 / 2",
//!   "output": 2,
//!   "taskLocals": {
//!     "RequestIdInstance": {
//!       "data": "87ca5d23-7d18-4485-b0c1-bff48a67a9a4",
//!       "scope": "Internal"
//!     },
//!     "note": "this data is injected after warp service ran",
//!   }
//! }
//! ```
//!
//! You can also attach a `-H 'x-request-id: my-external-request-id'` header and see the expected result:
//!
//! ```txt
//! HTTP/1.1 200 OK
//! content-type: application/json
//! x-request-id: my-external-request-id
//! content-length: 217
//! date: Mon, 20 Apr 2020 14:35:25 GMT
//!
//! {
//!   "op": "4 / 2",
//!   "output": 2,
//!   "taskLocals": {
//!     "RequestIdInstance": {
//!       "data": "my-external-request-id",
//!       "scope": "External"
//!     },
//!     "note": "this data is injected after warp service ran",
//!   }
//! }
//! ```
//!
#![deny(warnings)]

use hyper::{Body, Request, Response};
use log::{info, warn};
use serde::Serialize as SerializeTrait;
use serde_derive::Serialize;
use std::convert::Infallible;
use std::num::NonZeroU16;
use tower_service::Service;
use warp::{
    http::{HeaderMap, StatusCode},
    reject, Filter, Rejection, Reply,
};

// ===== custom request ID structure, note: all types must be Copy'able! =====

const REQUEST_ID_PREFIX_INTERNAL: &'static str = "internal-";
const REQUEST_ID_DATA_LENGTH: usize = 64; // usually sufficiently enough space for common request ID data
pub type InnerRequestIdData = [u8; REQUEST_ID_DATA_LENGTH];
pub type RequestIdData = arrayvec::ArrayString<InnerRequestIdData>;

#[derive(Debug, Copy, Clone, Serialize)]
enum RequestIdScope {
    Internal,
    External,
}

#[derive(Debug, Copy, Clone, Serialize)]
struct RequestId {
    scope: RequestIdScope,
    data: RequestIdData,
}

impl RequestId {
    fn to_string(&self) -> String {
        match self.scope {
            RequestIdScope::Internal => format!("{}{}", REQUEST_ID_PREFIX_INTERNAL, self.data),
            RequestIdScope::External => format!("{}", self.data), // external IDs do not get tampered with (other than truncation)
        }
    }

    fn generate_internal() -> Self {
        let uuid_string = uuid::Uuid::new_v4().to_hyphenated_ref().to_string();
        Self {
            scope: RequestIdScope::Internal,
            data: RequestIdData::from(&uuid_string).unwrap(),
        }
    }

    fn from_external(data: &str) -> Self {
        RequestId {
            scope: RequestIdScope::External,
            data: RequestIdData::from(data).unwrap(),
        }
    }

    // preferred and safe way to fill the array string, never allow external data to blow it up
    fn from_external_truncated(unbounded: &str) -> Self {
        // dirty way of getting the correct upper bound
        let min_length: usize = *[unbounded.len(), REQUEST_ID_DATA_LENGTH]
            .iter()
            .min()
            .unwrap(); // infallible at this point
        let (truncated, _) = unbounded.split_at(min_length);
        Self::from_external(truncated)
    }

    // try to get the header value and use a truncated version, otherwise fall back to internal if missing or parsing error
    fn from_headers_or_internal(headers: &HeaderMap) -> Self {
        match headers.get("x-request-id") {
            Some(hvalue) => match hvalue.to_str() {
                Ok(valid) => RequestId::from_external_truncated(valid),
                Err(_) => RequestId::generate_internal(),
            },
            None => RequestId::generate_internal(),
        }
    }
}

// the needed magic!
// could not find a better way to think about how to deal with data needed for the full request-response cycle
tokio::task_local! {
    static REQ_ID: RequestId;
}

// ===== MAIN =====
// mostly the rejection example code with some minor additions and changes for task local request ID usage

#[tokio::main]
async fn main() -> Result<(), hyper::error::Error> {
    pretty_env_logger::init();

    let math = warp::path!("math" / u16)
        .and(div_by())
        .map(|num: u16, denom: NonZeroU16| {
            let json = warp::reply::json(&Math {
                op: format!("{} / {}", num, denom),
                output: num / denom.get(),
            });
            json
        });

    let routes = warp::get()
        .and(math)
        .recover(handle_rejection)
        // we can access the task local and attach the header to our response with warp land:
        .map(|reply| warp::reply::with_header(reply, "x-request-id", REQ_ID.get().to_string()))
        .with(warp::log("app"));

    let mut warp_svc = warp::service(routes);
    let make_svc = hyper::service::make_service_fn(move |_| async move {
        let svc = hyper::service::service_fn(move |req: Request<Body>| async move {
            let request_id = RequestId::from_headers_or_internal(req.headers());
            REQ_ID
                .scope(request_id, async move {
                    info!("current request ID: {:?}", REQ_ID.get());
                    let warp_svc_response = warp_svc.call(req).await;
                    let (parts, body) = warp_svc_response.unwrap().into_parts();
                    // after example: attach request ID to body
                    let body = modify_body(body).await;
                    let rebuilt = Response::from_parts(parts, body);
                    Ok::<Response<Body>, Infallible>(rebuilt)
                })
                .await
        });
        Ok::<_, Infallible>(svc)
    });

    hyper::Server::bind(&([127, 0, 0, 1], 3030).into())
        .serve(make_svc)
        .await?;
    Ok(())
}

// same type out as in; you could add more arguments to use for body transformations
// like passing in a request ID which gets attached to a JSON property
async fn modify_body(body: hyper::body::Body) -> hyper::body::Body {
    let body_string = body_to_string(body).await;
    let mut json_value: serde_json::Value =
        serde_json::from_str(&body_string).expect("body must be valid JSON");

    // attach our task local data
    let json_object = json_value.as_object_mut().expect("value must be an object");
    json_object.insert(
        "taskLocals".into(),
        serde_json::json!({
            "note": "this data is injected after warp service ran",
            "RequestIdInstance": REQ_ID.get()
        }),
    );

    let final_body = print_json(&json_object);
    Body::from(final_body)
}

// this is mostly copy-pasta from the internet since I have zero idea how to easily collect the data;
// why do I have to make such a mess in the first place? a dbg!() showed it was just a
// single `Body { Full { ... } }` (so also on a single chunk containing all the data);
// I hope this really gets optimized away ...
async fn body_to_string(body: hyper::body::Body) -> String {
    use futures::TryStreamExt;
    let entire_body = body
        .try_fold(Vec::new(), |mut data, chunk| async move {
            data.extend_from_slice(&chunk);
            Ok(data)
        })
        .await
        .expect("body must be collectible into a Vec<u8>");
    String::from_utf8(entire_body).expect("body must be a valid UTF8 string")
}

// pretty and with final newline
pub fn print_json<T: ?Sized>(jsonable: &T) -> String
where
    T: SerializeTrait,
{
    let mut output =
        serde_json::to_string_pretty(jsonable).expect("print_json failed to stringify data");
    output.push('\n');
    output
}

// --- from rejection example again, only 2 tiny additions for the request ID here ---

#[derive(Debug, Serialize)]
struct DivideByZero;

impl reject::Reject for DivideByZero {}

#[derive(Serialize)]
struct Math {
    op: String,
    output: u16,
}
#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
    request_id: String, // added!
}

fn div_by() -> impl Filter<Extract = (NonZeroU16,), Error = Rejection> + Copy {
    warp::header::<u16>("div-by").and_then(|n: u16| async move {
        if let Some(denom) = NonZeroU16::new(n) {
            Ok(denom)
        } else {
            Err(reject::custom(DivideByZero))
        }
    })
}

// the custom rejection handler where we want to use our request ID
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "NOT_FOUND";
    } else if let Some(_) = err.find::<DivideByZero>() {
        code = StatusCode::BAD_REQUEST;
        message = "DIVIDE_BY_ZERO";
    } else if let Some(_) = err.find::<warp::reject::MethodNotAllowed>() {
        code = StatusCode::METHOD_NOT_ALLOWED;
        message = "METHOD_NOT_ALLOWED";
    } else {
        warn!("unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "UNHANDLED_REJECTION";
    }

    let json = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message: message.into(),
        request_id: REQ_ID.get().to_string(), // added!
    });
    let reply_with_status = warp::reply::with_status(json, code);
    Ok(reply_with_status)
}
