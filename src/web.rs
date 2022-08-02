use actix_web::middleware::Logger;
use actix_web::{get, App, HttpResponse, HttpServer, Responder};

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

pub fn serve() {
    let addr = "127.0.0.1:9999";
    let _s = HttpServer::new(|| App::new().wrap(Logger::default()).service(index))
        .bind(addr)
        .expect("fail bind")
        .shutdown_timeout(0)
        .run();

    info!("listen to {}", addr);
}
