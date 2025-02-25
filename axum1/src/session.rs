//! Cookie based session middleware for axum applications.
//!
//! [`SessionLayer`] provides client sessions via `async_session`.
//!  Sessions are backed by cryptographically signed cookies. These cookies
//! are generated when they’re not found or otherwise invalid. When a valid,
//! known cookie is received in a request, the session is hydrated from this
//! cookie. The middleware leverages `http::Extensions` to attach an
//! `async_session::Session` to the request. Request handlers can then
//! interact with the session:
//!
//! ```rust
//! use async_session::Session;
//! use axum::{Extension , http::StatusCode, response::IntoResponse};
//!
//! async fn handler(Extension(session): Extension<Session>) -> impl IntoResponse {
//!     match session.get::<String>("key") {
//!         Some(value) => Ok(value),
//!         _ => Err(StatusCode::NOT_FOUND),
//!     }
//! }
//! ```

use std::{
    task::{Context, Poll},
    time::Duration,
};

use async_session::{
    base64,
    hmac::{Hmac, Mac, NewMac},
    sha2::Sha256,
    SessionStore,
};
use axum::{
    body::Body,
    http::{
        header::{HeaderValue, COOKIE, SET_COOKIE},
        Request, StatusCode,
    },
    response::Response,
};
use axum_extra::extract::cookie::{Cookie, Key, SameSite};
use futures::future::BoxFuture;
use tower::{Layer, Service};

const BASE64_DIGEST_LEN: usize = 44;

#[derive(Clone)]
pub struct SessionLayer<Store> {
    store: Store,
    cookie_path: String,
    cookie_name: String,
    cookie_domain: Option<String>,
    session_ttl: Option<Duration>,
    save_unchanged: bool,
    same_site_policy: SameSite,
    secure: Option<bool>,
    key: Key,
}

impl<Store: SessionStore> SessionLayer<Store> {
    /// Creates a layer which will attach an [`async_session::Session`] to
    /// requests via an extension. This session is derived from a
    /// cryptographically signed cookie. When the client sends a valid,
    /// known cookie then the session is hydrated from this. Otherwise a new
    /// cookie is created and returned in the response.
    ///
    /// # Panics
    ///
    /// `SessionLayer::new` will panic if the secret is less than 64 bytes.
    pub fn new(store: Store, secret: &[u8]) -> Self {
        Self {
            store,
            save_unchanged: true,
            cookie_path: "/".into(),
            cookie_name: "axum_sid".into(),
            cookie_domain: None,
            same_site_policy: SameSite::Strict,
            session_ttl: Some(Duration::from_secs(24 * 60 * 60)),
            secure: None,
            key: Key::from(secret),
        }
    }

    /// When `true`, a session cookie will always be set. When `false` the
    /// session data must be modified in order for it to be set. Defaults to
    /// `true`.
    pub fn with_save_unchanged(mut self, save_unchanged: bool) -> Self {
        self.save_unchanged = save_unchanged;
        self
    }

    /// Sets a cookie for the session. Defaults to `"/"`.
    pub fn with_cookie_path(mut self, cookie_path: impl AsRef<str>) -> Self {
        self.cookie_path = cookie_path.as_ref().to_owned();
        self
    }

    /// Sets a cookie name for the session. Defaults to `"axum_sid"`.
    pub fn with_cookie_name(mut self, cookie_name: impl AsRef<str>) -> Self {
        self.cookie_name = cookie_name.as_ref().to_owned();
        self
    }

    /// Sets a cookie domain for the session. Defaults to `None`.
    pub fn with_cookie_domain(mut self, cookie_domain: impl AsRef<str>) -> Self {
        self.cookie_domain = Some(cookie_domain.as_ref().to_owned());
        self
    }

    /// Sets a cookie same site policy for the session. Defaults to
    /// `SameSite::Strict`.
    pub fn with_same_site_policy(mut self, policy: SameSite) -> Self {
        self.same_site_policy = policy;
        self
    }

    /// Sets a cookie time-to-live (ttl) for the session. Defaults to
    /// `Duration::from_secs(60 * 60 24)`; one day.
    pub fn with_session_ttl(mut self, session_ttl: Option<Duration>) -> Self {
        self.session_ttl = session_ttl;
        self
    }

    /// Sets a cookie secure attribute for the session. Defaults to `false`.
    pub fn with_secure(mut self, secure: bool) -> Self {
        self.secure = Some(secure);
        self
    }

    async fn load_or_create(&self, cookie_value: Option<String>) -> crate::session_ext::Session {
        let session = match cookie_value {
            Some(cookie_value) => self.store.load_session(cookie_value).await.ok().flatten(),
            None => None,
        };

        let inner = session
            .and_then(|session| session.validate())
            .unwrap_or_default();

        crate::session_ext::Session::from_inner(inner)
    }

    fn build_cookie(&self, secure: bool, cookie_value: String) -> Cookie<'static> {
        let mut cookie = Cookie::build(self.cookie_name.clone(), cookie_value)
            .http_only(true)
            .same_site(self.same_site_policy)
            .secure(secure)
            .path(self.cookie_path.clone())
            .finish();

        if let Some(ttl) = self.session_ttl {
            cookie.set_expires(Some((std::time::SystemTime::now() + ttl).into()));
        }

        if let Some(cookie_domain) = self.cookie_domain.clone() {
            cookie.set_domain(cookie_domain)
        }

        self.sign_cookie(&mut cookie);

        cookie
    }

    fn build_removal_cookie(&self, secure: bool) -> Cookie<'static> {
        let mut cookie = Cookie::build(self.cookie_name.clone(), "")
            .http_only(true)
            .same_site(self.same_site_policy)
            .secure(secure)
            .finish();

        cookie.make_removal();

        self.sign_cookie(&mut cookie);

        cookie
    }

    // This is mostly based on:
    // https://github.com/SergioBenitez/cookie-rs/blob/master/src/secure/signed.rs#L33-L43
    /// Signs the cookie's value providing integrity and authenticity.
    fn sign_cookie(&self, cookie: &mut Cookie<'_>) {
        // Compute HMAC-SHA256 of the cookie's value.
        let mut mac = Hmac::<Sha256>::new_from_slice(self.key.signing()).expect("a good key");
        mac.update(cookie.value().as_bytes());

        // Cookie's new value is [MAC | original-value].
        let mut new_value = base64::encode(mac.finalize().into_bytes());
        new_value.push_str(cookie.value());
        cookie.set_value(new_value);
    }

    // This is mostly based on:
    // https://github.com/SergioBenitez/cookie-rs/blob/master/src/secure/signed.rs#L45-L63
    /// Given a signed value `str` where the signature is prepended to `value`,
    /// verifies the signed value and returns it. If there's a problem, returns
    /// an `Err` with a string describing the issue.
    fn verify_signature(&self, cookie_value: &str) -> Result<String, &'static str> {
        if cookie_value.len() < BASE64_DIGEST_LEN {
            return Err("length of value is <= BASE64_DIGEST_LEN");
        }

        // Split [MAC | original-value] into its two parts.
        let (digest_str, value) = cookie_value.split_at(BASE64_DIGEST_LEN);
        let digest = base64::decode(digest_str).map_err(|_| "bad base64 digest")?;

        // Perform the verification.
        let mut mac = Hmac::<Sha256>::new_from_slice(self.key.signing()).expect("a good key");
        mac.update(value.as_bytes());
        mac.verify(&digest)
            .map(|_| value.to_string())
            .map_err(|_| "value did not verify")
    }
}

impl<S, Store: SessionStore> Layer<S> for SessionLayer<Store> {
    type Service = Session<S, Store>;

    fn layer(&self, inner: S) -> Self::Service {
        Session {
            inner,
            layer: self.clone(),
        }
    }
}

#[derive(Clone)]
pub struct Session<S, Store: SessionStore> {
    inner: S,
    layer: SessionLayer<Store>,
}

impl<S, Store: SessionStore> Service<Request<Body>> for Session<S, Store>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let session_layer = self.layer.clone();

        let cookie_values = request
            .headers()
            .get(COOKIE)
            .map(|cookies| cookies.to_str());

        let cookie_value = if let Some(Ok(cookies)) = cookie_values {
            cookies
                .split(';')
                .map(|cookie| cookie.trim())
                .filter_map(|cookie| Cookie::parse_encoded(cookie).ok())
                .filter(|cookie| cookie.name() == session_layer.cookie_name)
                .find_map(|cookie| self.layer.verify_signature(cookie.value()).ok())
        } else {
            None
        };

        let secure = self
            .layer
            .secure
            .unwrap_or_else(|| request.uri().scheme_str() == Some("https"));

        let not_ready_service = self.inner.clone();
        let mut ready_service = std::mem::replace(&mut self.inner, not_ready_service);

        Box::pin(async move {
            let mut session = session_layer.load_or_create(cookie_value.clone()).await;

            if let Some(ttl) = session_layer.session_ttl {
                session.expire_in(ttl);
            }

            request.extensions_mut().insert(session.clone());

            let mut response: Response = ready_service.call(request).await?;

            if session.is_destroyed() {
                if let Err(e) = session_layer
                    .store
                    .destroy_session(session.into_inner())
                    .await
                {
                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    tracing::error!("Failed to destroy session: {:?}", e);
                }

                let removal_cookie = session_layer.build_removal_cookie(secure);

                response.headers_mut().insert(
                    SET_COOKIE,
                    HeaderValue::from_str(&removal_cookie.to_string()).unwrap(),
                );
            } else if session_layer.save_unchanged
                || session.data_changed()
                || cookie_value.is_none()
            {
                if session.should_regenerate() {
                    if let Err(e) = session_layer
                        .store
                        .destroy_session(session.clone().into_inner())
                        .await
                    {
                        tracing::error!("Failed to destroy old session on regenerate: {:?}", e);
                    }
                    session.inner_regenerate();
                }
                match session_layer
                    .store
                    .store_session(session.into_inner())
                    .await
                {
                    Ok(Some(cookie_value)) => {
                        let cookie = session_layer.build_cookie(secure, cookie_value);
                        response.headers_mut().insert(
                            SET_COOKIE,
                            HeaderValue::from_str(&cookie.to_string()).unwrap(),
                        );
                    }
                    Ok(None) => {}
                    Err(e) => {
                        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                        tracing::error!("Failed to reach session storage {:?}", e);
                    }
                }
            }

            Ok(response)
        })
    }
}
