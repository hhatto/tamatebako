use actix;
use actix_web::{middleware, server, App, HttpRequest};

use database;

fn index(_req: &HttpRequest) -> &'static str {
    "Hello world!"
}

pub fn serve() {
    let sys = actix::System::new("tamatebako-web");
    let addr = "127.0.0.1:9999";
    let _s = server::new(|| {
        App::new()
        .middleware(middleware::Logger::default())
        .resource("/", |r| r.f(index))
    })
    .bind(addr).expect("fail bind")
    .shutdown_timeout(0)
    .start();

    info!("listen to {}", addr);
    let _ = sys.run();
}
