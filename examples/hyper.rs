use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use webdav_handler::{fakels::FakeLs, localfs::LocalFs, DavHandler};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::init();
    let dir = "/tmp";
    let addr = SocketAddr::from(([127, 0, 0, 1], 4918));

    let dav_server = DavHandler::builder()
        .filesystem(LocalFs::new(dir, false, false, false))
        .locksystem(FakeLs::new())
        .autoindex(true, None)
        .build_handler();

    let listener = TcpListener::bind(addr).await?;
    println!("hyper example: listening on {:?} serving {}", addr, dir);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let dav_server = dav_server.clone();
        tokio::task::spawn(async move {
            if let Err(err) = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        let dav_server = dav_server.clone();
                        async move { Ok::<_, Infallible>(dav_server.handle(req).await) }
                    }),
                )
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
