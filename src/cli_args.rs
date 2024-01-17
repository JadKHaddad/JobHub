use clap::Parser;
use std::net::SocketAddr;

#[derive(Parser)]
#[command(author, about, version)]
pub struct CliArgs {
    /// The address to bind the server to
    #[clap(long, env = "SOCKET_ADDRESS", default_value = "127.0.0.1:3000")]
    pub socket_address: SocketAddr,

    /// The API token to use for authentication
    #[clap(long, env = "API_TOKEN")]
    pub api_token: String,
}
