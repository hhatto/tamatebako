use actix_web::{web, HttpServer, App, HttpResponse, Responder};
use actix_web::middleware::Logger;

fn index() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

pub fn serve() {
    let addr = "127.0.0.1:9999";
    let _s = HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .service(
                web::resource("/").to(index)
            )
    })
    .bind(addr)
    .expect("fail bind")
    .shutdown_timeout(0)
    .start();

    info!("listen to {}", addr);
}
