// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io::Read;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Parser;
use rusb::GlobalContext;
use wasefire_protocol::service::applet::{self, AppletId};
use wasefire_protocol::{Api, Request, Response};
use wasefire_protocol_usb::{self as rpc, Connection};

mod tests;

#[derive(Parser)]
enum Flags {
    /// Starts a request/response call with an applet.
    Call,

    /// Starts a tunnel with a given delimiter.
    ///
    /// The delimiter is automatically sent when standard input is closed. The tunnel is line-based.
    Tunnel { delimiter: String },

    /// Runs tests for this applet (this is not a protocol command).
    Test,
}

fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let flags = Flags::parse();
    let candidate = rpc::choose_device().context("choosing device")?;
    let connection = candidate.connect().context("connecting to the device")?;
    match flags {
        Flags::Call => {
            let mut request = Vec::new();
            std::io::stdin().read_to_end(&mut request)?;
            let request = applet::Request { applet_id: AppletId, request: &request };
            send(&connection, &Api::<Request>::AppletRequest(request))?;
            receive(&connection, |response| Ok(matches!(response, Api::AppletRequest(()))))?;
            send(&connection, &Api::<Request>::AppletResponse(AppletId))?;
            receive(&connection, |response| match response {
                Api::AppletResponse(applet::Response { response }) => {
                    let response = response.context("no applet response")?;
                    print!("{}", std::str::from_utf8(response).unwrap());
                    Ok(true)
                }
                _ => Ok(false),
            })
        }
        Flags::Tunnel { delimiter } => {
            let delimiter = delimiter.as_bytes();
            let tunnel = applet::Tunnel { applet_id: applet::AppletId, delimiter };
            send(&connection, &Api::<Request>::AppletTunnel(tunnel))?;
            read_tunnel(&connection)?;
            for line in std::io::stdin().lines() {
                let request = line.context("reading line")?.into_bytes();
                if request == delimiter {
                    break;
                }
                connection.send(&request, TIMEOUT).context("sending request")?;
                let response = connection.receive(TIMEOUT).context("receiving response")?;
                println!("{}", String::from_utf8(response).unwrap());
            }
            connection.send(delimiter, TIMEOUT)?;
            read_tunnel(&connection)
        }
        Flags::Test => tests::main(&connection),
    }
}

const TIMEOUT: Duration = Duration::from_secs(1);

fn send(connection: &Connection<GlobalContext>, request: &Api<Request>) -> Result<()> {
    let request = request.serialize();
    connection.send(&request, TIMEOUT).context("sending request")?;
    Ok(())
}

fn receive(
    connection: &Connection<GlobalContext>, process: impl FnOnce(&Api<Response>) -> Result<bool>,
) -> Result<()> {
    let response = connection.receive(TIMEOUT).context("receiving response")?;
    let response = Api::<Response>::deserialize(&response).context("deserializing response")?;
    if !process(&response)? {
        match response {
            Api::DeviceError(error) => bail!("error response: {error}"),
            _ => bail!("invalid response: {response:?}"),
        }
    }
    Ok(())
}

fn read_tunnel(connection: &Connection<GlobalContext>) -> Result<()> {
    receive(connection, |x| Ok(matches!(x, Api::AppletTunnel(()))))
}
