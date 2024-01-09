use std::net::SocketAddr;
use webdav_handler::warp::dav_dir;

#[tokio::main]
async fn main() {
    env_logger::init();
    let dir = "/tmp";
    let addr: SocketAddr = ([127, 0, 0, 1], 4918).into();

    println!("warp example: listening on {:?} serving {}", addr, dir);
    let warpdav = dav_dir(dir, true, true, None);
    warp::serve(warpdav).run(addr).await;
}
