mod io;
use futures::{
    future::select,
    pin_mut,
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use io::*;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

const BINARIES_URL: &str = "https://github.com/lightvector/KataGo/releases/download/v1.13.0/";
const BINARY_DIR: &str = "KataGo";
#[cfg(windows)]
mod platform {
    pub const BINARY_GPU_ZIP: &str = "katago-v1.13.0-opencl-windows-x64.zip";
    pub const BINARY_CPU_ZIP: &str = "katago-v1.13.0-eigenavx2-windows-x64.zip";
    pub const BINARY: &str = "katago.exe";
}
#[cfg(unix)]
mod platform {
    pub const BINARY_GPU_ZIP: &str = "katago-v1.13.0-opencl-linux-x64.zip";
    pub const BINARY_CPU_ZIP: &str = "katago-v1.13.0-eigenavx2-linux-x64.zip";
    pub const BINARY: &str = "katago";
}
const MODELS_URL: &str = "https://media.katagotraining.org/uploaded/networks/models/kata1/";
const MODEL: &str = "kata1-b18c384nbt-s8341979392-d3881113763.bin.gz";

fn use_gpu_binary() -> bool {
    println!("Choose KataGo version:");
    println!("1. GPU (OpenCL)");
    println!("2. CPU (Eigen)");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");
    match input.trim().parse::<u32>() {
        Ok(1) => {
            println!("Using GPU (OpenCL) KataGo version.");
            true
        }
        Ok(2) => {
            println!("Using CPU (Eigen) KataGo version.");
            false
        }
        _ => {
            println!("Invalid choice. Please enter 1 or 2.");
            use_gpu_binary()
        }
    }
}

fn string_path(a: &str, b: &str) -> String {
    PathBuf::from(a).join(b).display().to_string()
}

async fn run() -> tokio::process::Child {
    let binary_path = string_path(BINARY_DIR, platform::BINARY);
    let model_path = MODEL.to_owned();
    ensure("KataGo binary".to_owned(), &binary_path, move |path| {
        Box::pin(async move {
            let binary_zip = if use_gpu_binary() {
                platform::BINARY_GPU_ZIP
            } else {
                platform::BINARY_CPU_ZIP
            };
            println!("Downloading KataGo");
            download_file(BINARIES_URL.to_owned() + binary_zip, binary_zip).await?;
            println!("Unpacking KataGo");
            unzip(binary_zip, BINARY_DIR).map_err(DownloadError::IO)?;
            println!("Removing Zip");
            std::fs::remove_file(binary_zip).map_err(DownloadError::IO)?;
            println!("Setting execution permission");
            set_exe_permission(&path).map_err(DownloadError::IO)
        })
    })
    .await;
    ensure(format!("Model {MODEL}"), &model_path, move |_| {
        Box::pin(async move {
            println!("Downloading model {MODEL}");
            download_file(MODELS_URL.to_owned() + MODEL, MODEL).await
        })
    })
    .await;
    let config = string_path(BINARY_DIR, "analysis_example.cfg");
    let child = Command::new(format!("./{binary_path}"))
        .arg("analysis")
        .arg("-model")
        .arg(model_path)
        .arg("-config")
        .arg(config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start binary");
    println!("Running binary {binary_path}");
    child
}

type WS = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

async fn binary_to_ws(mut stdout: ChildStdout, mut socket: SplitSink<WS, Message>) {
    let mut buffer = [0; 1024];
    loop {
        match stdout.read(&mut buffer).await {
            Ok(0) => break,
            Ok(bytes_read) => {
                let xs = buffer[..bytes_read].to_vec();
                let line = String::from_utf8(xs).expect("Error parsing UTF-8");
                print!("binary: {}", line.clone());
                socket
                    .send(Message::Text(line.clone()))
                    .await
                    .expect("Failed to send");
            }
            Err(err) => eprintln!("Error reading from stdout: {:?}", err),
        }
    }
}

async fn ws_to_binary(mut stdin: ChildStdin, mut socket: SplitStream<WS>) {
    loop {
        match socket.next().await.expect("Failed to receive") {
            Ok(msg) => match msg {
                Message::Text(line) => {
                    print!("socket: {}", line);
                    stdin
                        .write_all(line.as_bytes())
                        .await
                        .expect("Error writing to stdin");
                    stdin.flush().await.expect("Error flushing to stdin");
                }
                Message::Close(_) => break,
                _ => eprintln!("Not text or close message {}", msg),
            },
            Err(err) => eprintln!("{}", err),
        }
    }
}

async fn communicate(mut binary: tokio::process::Child) {
    let url = std::env::args()
        .nth(1)
        .expect("this program requires url to connect as argument");
    let url = url::Url::parse(&url).expect("provided url doesn't parse");
    let (socket, _response) = connect_async(url.clone()).await.expect("can't connect");
    let (send, receive) = socket.split();
    println!("Connection with {url} established");
    let stdout = binary.stdout.take().expect("Failed to open stdout");
    let stdin = binary.stdin.take().expect("Failed to open stdin");
    let a = binary_to_ws(stdout, send);
    let b = ws_to_binary(stdin, receive);
    pin_mut!(a, b);
    select(a, b).await;
}

#[tokio::main]
async fn main() {
    communicate(run().await).await;
}
