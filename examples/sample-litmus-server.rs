//
//  Sample application.
//
//  Listens on localhost:4918, plain http, no ssl.
//  Connect to http://localhost:4918/
//

use std::convert::Infallible;
use std::error::Error;
use std::net::SocketAddr;
use std::str::FromStr;

use clap::Parser;
use env_logger;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;

use headers::{authorization::Basic, Authorization, HeaderMapExt};

use webdav_handler::{fakels, localfs, memfs, memls, DavConfig, DavHandler};
use webdav_handler::{body::Body, time::UtcOffset};

#[derive(Clone)]
struct Server {
    dh:   DavHandler,
    auth: bool,
}

impl Server {
    pub fn new(directory: String, memls: bool, fakels: bool, auth: bool) -> Self {
        let mut config = DavHandler::builder();
        if directory != "" {
            let utctime = time::UtcOffset::current_local_offset().map(UtcOffset::from).ok();
            config = config
                .filesystem(localfs::LocalFs::new(directory, true, true, true))
                .autoindex(true, utctime);
        } else {
            config = config.filesystem(memfs::MemFs::new());
        };
        if fakels {
            config = config.locksystem(fakels::FakeLs::new());
        }
        if memls {
            config = config.locksystem(memls::MemLs::new());
        }

        Server {
            dh: config.build_handler(),
            auth,
        }
    }

    async fn handle(
        &self,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Response<Body>, Infallible> {
        let user = if self.auth {
            // we want the client to authenticate.
            match req.headers().typed_get::<Authorization<Basic>>() {
                Some(Authorization(basic)) => Some(basic.username().to_string()),
                None => {
                    // return a 401 reply.
                    let response = hyper::Response::builder()
                        .status(401)
                        .header("WWW-Authenticate", "Basic realm=\"foo\"")
                        .body(Body::from("please auth".to_string()))
                        .unwrap();
                    return Ok(response);
                },
            }
        } else {
            None
        };

        if let Some(user) = user {
            let config = DavConfig::new().principal(user);
            Ok(self.dh.handle_with(config, req).await)
        } else {
            Ok(self.dh.handle(req).await)
        }
    }
}

#[derive(Parser)]
struct Args {
    /// port to listen on (4918)
    #[arg(short, long, default_value_t = 4918)]
    port:   u16,
    /// local directory to serve (default: current dir)
    #[arg(short, long, default_value_t = String::new())]
    dir:    String,
    /// serve from ephemeral memory filesystem (default)
    #[arg(short, long, default_value_t = true)]
    memfs:  bool,
    /// use ephemeral memory locksystem (default with --memfs)
    #[arg(short = 'l', long, default_value_t = false)]
    memls:  bool,
    /// use fake memory locksystem (default with --memfs)
    #[arg(short, long, default_value_t = false)]
    fakels: bool,
    /// require basic authentication
    #[arg(short, long, default_value_t = false)]
    auth:   bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let args = Args::parse();

    let (dir, name) = match args.dir.as_str() {
        "" => ("", "memory filesystem"),
        dir => (dir, dir),
    };
    let memls = args.memfs || args.memls;
    let fakels = args.fakels;
    let auth = args.auth;
    let addr = format!("0.0.0.0:{}", args.port);
    let addr = SocketAddr::from_str(&addr)?;
    let dav_server = Server::new(dir.to_string(), memls, fakels, auth);

    let listener = TcpListener::bind(addr).await?;
    println!("Serving {} on {}", name, args.port);

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
                        async move { dav_server.handle(req).await }
                    }),
                )
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
