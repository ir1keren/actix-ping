use actix_web::{middleware, web, App, HttpResponse, HttpServer, Responder};
use tokio::sync::oneshot;
use std::{net::IpAddr, str::FromStr, env, process::Command, time::Duration};

const IP_ADDRESS:&'static str="192.168.1.125";

fn do_something()
{
    println!("Ok. Doing something");
}

async fn is_server_running()->bool
{
    if let Ok(val)=reqwest::get("http://127.0.0.1/no-op").await {
        val.text().await.map(|s|&s=="true").unwrap_or(false)
    } else {
        false
    }
}

async fn no_op() -> impl Responder
{
    HttpResponse::Ok().content_type("text/plain").body("true")
}

async fn start_ping()
{
   let ip=IpAddr::from_str(IP_ADDRESS).unwrap();
   let options = ping_rs::PingOptions { ttl: 128, dont_fragment: true };
   let data=[1,2,3,4];

    while is_server_running().await {
        println!("Pinging {}",IP_ADDRESS);
        match ping_rs::send_ping(&ip, Duration::from_secs(30), &data, (&options).into()) {
            Err(err)=>eprintln!("Error: {:?}",err), //Error here on Ubuntu,
            _=>do_something()
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
}

pub async fn start_server() -> anyhow::Result<()>
{
    let bind_ip=match env::var("APP_BIND_IP") {
        Ok(val)=>val,
        _=>"0.0.0.0".to_string()
    };
    let http_port=match env::var("HTTP_PORT") {
        Ok(val)=>val.parse::<u32>().unwrap_or(80),
        _=>80_u32
    };
    let http_server=HttpServer::new(move || {
       App::new().wrap(middleware::NormalizePath::new(middleware::TrailingSlash::Trim))
       .route("/no-op", web::get().to(no_op))
    });
    let (tx,rx)=oneshot::channel::<()>();
    // spawns a new thread and then a new process
    // to periodically "ping"
    actix::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        println!("Spawning child");
        let current_dir=std::env::current_dir().unwrap();
        let current_exe=std::env::current_exe().unwrap();
        match Command::new(&current_exe).current_dir(&current_dir).arg("ping").spawn() {
            Err(err)=>{
                eprintln!("Failed spawning child: {}",err);
            },
            Ok(mut n)=>{
                let _=rx.await;
                let _=n.kill();
            }
        }
    });

    http_server.bind(format!("{0}:{1}",bind_ip,http_port))?.run().await?;
    let _=tx.send(());
    Ok(())
}

fn main()->anyhow::Result<()> {
    let args=env::args().collect::<Vec<String>>();
    let ping_cmd="ping".to_string();
    //Found ping argument
    if args.contains(&ping_cmd) {
        tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async move {
            start_ping().await;
        });
    } else {
        actix::System::new().block_on(async move {
            start_server().await
        })?;
    }

    Ok(())
}
