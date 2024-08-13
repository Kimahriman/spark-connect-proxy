use axum::response::IntoResponse;
use futures_util::future::BoxFuture;
use http::{header::AUTHORIZATION, Request, Response, StatusCode};
use log::info;
use tower_http::auth::AsyncAuthorizeRequest;

#[derive(Clone)]
pub struct UserId(pub String);

#[derive(Clone)]
pub struct BearerToken(pub String);

#[derive(Clone)]
pub struct UserAuth {}

impl AsyncAuthorizeRequest<axum::body::Body> for UserAuth {
    type RequestBody = axum::body::Body;

    type ResponseBody = axum::body::Body;

    type Future =
        BoxFuture<'static, Result<Request<Self::RequestBody>, Response<Self::ResponseBody>>>;

    fn authorize(&mut self, mut request: hyper::Request<axum::body::Body>) -> Self::Future {
        Box::pin(async {
            request
                .extensions_mut()
                .insert(UserId("testuser".to_string()));
            Ok(request)
        })
    }
}

#[derive(Clone)]
pub struct TokenAuth {}

impl AsyncAuthorizeRequest<axum::body::Body> for TokenAuth {
    type RequestBody = axum::body::Body;

    type ResponseBody = axum::body::Body;

    type Future =
        BoxFuture<'static, Result<Request<Self::RequestBody>, Response<Self::ResponseBody>>>;

    fn authorize(&mut self, mut request: hyper::Request<Self::RequestBody>) -> Self::Future {
        Box::pin(async {
            let authorization = request
                .headers()
                .get(AUTHORIZATION)
                .ok_or(StatusCode::UNAUTHORIZED.into_response())?
                .to_str()
                .map_err(|_| StatusCode::BAD_REQUEST.into_response())?
                .to_string();

            info!("Authorizing token: {}", authorization);

            let split = authorization.split_once(' ');
            let token = match split {
                Some(("Bearer", token)) => token,
                _ => return Err(StatusCode::UNAUTHORIZED.into_response()),
            };

            request
                .extensions_mut()
                .insert(BearerToken(token.to_string()));
            Ok(request)
        })
    }
}
