use futures_util::future::BoxFuture;
use http::{Request, Response};
use tower_http::auth::AsyncAuthorizeRequest;

#[derive(Clone)]
pub struct UserId(pub String);

#[derive(Clone)]
pub struct TestAuth {}

impl AsyncAuthorizeRequest<axum::body::Body> for TestAuth {
    type RequestBody = axum::body::Body;

    type ResponseBody = axum::body::Body;

    type Future =
        BoxFuture<'static, Result<Request<axum::body::Body>, Response<Self::ResponseBody>>>;

    fn authorize(&mut self, mut request: hyper::Request<axum::body::Body>) -> Self::Future {
        Box::pin(async {
            request
                .extensions_mut()
                .insert(UserId("testuser".to_string()));
            Ok(request)
        })
    }
}
