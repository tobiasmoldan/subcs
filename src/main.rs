use anyhow::Result;
use async_std::net::{SocketAddr, UdpSocket};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::str::from_utf8;
use std::time::{Duration, Instant};

#[async_std::main]
async fn main() -> Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:8000").await?;

    let mut buf = [0u8; 4096];

    let mut users = HashMap::<SocketAddr, Instant>::new();
    let mut del_list = Vec::new();

    const MIN_DUR: Duration = Duration::from_millis(333);
    const MAX_DUR: Duration = Duration::from_secs(300);

    while let Ok((size, addr)) = socket.recv_from(&mut buf).await {
        match users.entry(addr) {
            Entry::Vacant(v) => {
                v.insert(Instant::now());
            }
            Entry::Occupied(mut o) => {
                *o.get_mut() = Instant::now();
            }
        }

        for (k, v) in users.iter() {
            if k == &addr {
                continue;
            }
            if v.elapsed() > MAX_DUR {
                del_list.push(k.clone());
            } else if v.elapsed() >= MIN_DUR {
                socket.send_to(&buf[..size], k).await?;
            }
        }

        while let Some(del) = del_list.pop() {
            println!("deleting {:?}", del);
            users.remove(&del);
        }

        println!("{:?}", users);

        if buf[size - 1] == b'\n' {
            println!(
                "{}:{}> {}",
                addr.ip(),
                addr.port(),
                from_utf8(&buf[..size - 1])?
            );
        } else {
            println!(
                "{}:{}> {}",
                addr.ip(),
                addr.port(),
                from_utf8(&buf[..size])?
            );
        }
    }

    Ok(())
}
