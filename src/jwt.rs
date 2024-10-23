use std::{rc::Rc, task::{Context, Poll}};
use actix_web::Error;
use actix_service::{Service, Transform};
use actix_web::dev::ServiceRequest;
use futures::{future::{ok, LocalBoxFuture, Ready}, FutureExt};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{decode, encode, errors::Result as JwtResult, Algorithm, DecodingKey, EncodingKey, Header, Validation};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

pub fn generate_jwt(user_id: &str) -> JwtResult<String> {
    let expiration: usize = 10000;
    let claims: Claims = Claims {
        sub: user_id.to_string(),
        exp: (chrono::Utc::now().timestamp() as usize) + expiration,
    };

    let secret_key: &str = "your_secret_key"; 
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret_key.as_ref()))
}

pub struct JwtMiddleware;

impl<S, B> Transform<S, ServiceRequest> for JwtMiddleware
where
    S: Service<ServiceRequest, Response = actix_web::dev::ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Response = actix_web::dev::ServiceResponse<B>;
    type Error = Error;
    type Transform = JwtMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(JwtMiddlewareService {
            service: Rc::new(service),
        })
    }
}

pub struct JwtMiddlewareService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for JwtMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = actix_web::dev::ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Response = actix_web::dev::ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service: Rc<S> = Rc::clone(&self.service);
        
        if let Some(auth_header) = req.headers().get("Authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("Bearer ") {
                    let token: &str = &auth_str[7..];
                    
                    let secret_key: &str = "your_secret_key";
                    let validation: Validation = Validation::new(Algorithm::HS256);

                    let token_data: Result<jsonwebtoken::TokenData<Claims>, jsonwebtoken::errors::Error> = decode::<Claims>(
                        token,
                        &DecodingKey::from_secret(secret_key.as_ref()),
                        &validation,
                    );

                    match token_data {
                        Ok(_data) => {
                            return service.call(req).boxed_local();
                        }
                        Err(_err) => {
                            return Box::pin(async {
                                Err(actix_web::error::ErrorUnauthorized("Invalid token"))
                            });
                        }
                    }
                }
            }
        }
        Box::pin(async {
            Err(actix_web::error::ErrorUnauthorized("Authorization header missing or invalid"))
        })
    }
}