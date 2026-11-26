use std::net::SocketAddr;

use axum::body::to_bytes;
use axum::extract::ConnectInfo;

use super::*;

pub fn router(state: &AppState<AppBackend>) -> Router {
    build_router(state.clone())
}

pub fn request(method: Method, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4242))))
        .body(body)
        .expect("requete")
}

pub fn get(uri: &str) -> Request<Body> {
    request(Method::GET, uri, Body::empty())
}

pub fn post_form(uri: &str, body: &str) -> Request<Body> {
    let mut request = request(Method::POST, uri, Body::from(body.to_string()));
    request.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/x-www-form-urlencoded"),
    );
    request
}

pub fn with_cookie(mut request: Request<Body>, token: &str) -> Request<Body> {
    request.headers_mut().insert(
        header::COOKIE,
        header::HeaderValue::from_str(&format!("fs-ez_session={token}")).expect("cookie"),
    );
    request
}

pub fn with_site(mut request: Request<Body>, site: &'static str) -> Request<Body> {
    request
        .headers_mut()
        .insert("sec-fetch-site", header::HeaderValue::from_static(site));
    request
}

pub fn with_hx(mut request: Request<Body>) -> Request<Body> {
    request
        .headers_mut()
        .insert("hx-request", header::HeaderValue::from_static("true"));
    request
}

pub fn with_referer(mut request: Request<Body>, target: &str) -> Request<Body> {
    request.headers_mut().insert(
        header::REFERER,
        header::HeaderValue::from_str(target).expect("referer"),
    );
    request
}

pub fn login_body(password: &str) -> String {
    format!("username={USERNAME}&password={password}")
}

pub async fn send(router: &Router, request: Request<Body>) -> Response<Body> {
    router.clone().oneshot(request).await.expect("reponse")
}

pub async fn body_text(response: Response<Body>) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("corps");
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn header_string(response: &Response<Body>, name: header::HeaderName) -> String {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

pub fn location(response: &Response<Body>) -> String {
    header_string(response, header::LOCATION)
}

pub fn session_token(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .and_then(|pair| pair.trim().strip_prefix("fs-ez_session="))
        .unwrap_or_default()
        .to_string()
}

pub async fn login(router: &Router) -> String {
    let response = send(router, post_form("/login", &login_body(PASSWORD))).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    session_token(&header_string(&response, header::SET_COOKIE))
}

pub async fn page(router: &Router, token: &str, uri: &str) -> (StatusCode, String) {
    let response = send(router, with_cookie(get(uri), token)).await;
    let status = response.status();
    (status, body_text(response).await)
}
