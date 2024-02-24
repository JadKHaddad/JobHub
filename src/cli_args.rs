use clap::Parser;
use std::net::SocketAddr;

#[derive(Parser)]
#[command(author, about, version)]
pub struct CliArgs {
    /// The address to bind the server to
    #[clap(long, env = "SOCKET_ADDRESS", default_value = "127.0.0.1:3000")]
    pub socket_address: SocketAddr,

    /// The public domains to use for the API
    #[clap(long, env = "SERVER_URLS", value_delimiter = ',')]
    pub server_urls: Vec<String>,

    /// The API token to use for authentication
    #[clap(long, env = "API_TOKEN")]
    pub api_token: String,

    /// The directory where the projects are located
    #[clap(long, env = "PROJECTS_DIR", default_value = "projects")]
    pub projects_dir: String,
}
