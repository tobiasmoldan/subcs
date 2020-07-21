use anyhow::Result;
use std::collections::{
    hash_map::Entry::{Occupied, Vacant},
    HashMap,
};
use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{delay_for, Duration, Instant};
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    let socket = UdpSocket::bind((Ipv4Addr::new(0, 0, 0, 0), 8000)).await?;

    let (mut udp_rx, mut udp_tx) = socket.split();

    let (exit_tx, mut exit_rx1) = broadcast::channel::<()>(1);
    let mut exit_rx2 = exit_tx.subscribe();
    let mut exit_rx3 = exit_tx.subscribe();

    let (mut out_tx, mut out_rx) = mpsc::channel::<(Vec<SocketAddr>, Vec<u8>)>(100);
    let (mut in_tx, mut in_rx) = mpsc::channel::<(SocketAddr, Vec<u8>)>(10);

    let rec = tokio::spawn(async move {
        let mut buf = [0u8; 4096];

        loop {
            tokio::select! {
                Ok(_) = exit_rx2.recv() => {
                    break;
                },
                Ok((size, addr)) = udp_rx.recv_from(&mut buf) => {
                    let mut vec = Vec::<u8>::with_capacity(size);
                    vec.extend_from_slice(&buf[..size]);
                    in_tx.send((addr, vec)).await.unwrap();
                }
            }
        }
    });

    let timeouts = tokio::spawn(async move {
        let mut connections = HashMap::<SocketAddr, Instant>::new();
        let mut del_list = Vec::<SocketAddr>::new();

        const MIN_TIME_DIFF: Duration = Duration::from_millis(333);
        const MAX_TIME_DIFF: Duration = Duration::from_secs(180);
        loop {
            tokio::select! {
                Ok(_) = exit_rx3.recv() => {
                    in_rx.close();
                },
                res = in_rx.recv() => {
                    match res {
                        None => {
                            break;
                        },
                        Some((addr, data)) => {

                            match connections.entry(addr) {
                                Occupied(mut o) => {
                                    if o.get().elapsed() < MIN_TIME_DIFF {
                                        continue;
                                    }
                                    *o.get_mut() = Instant::now();
                                },
                                Vacant(v) => {
                                    v.insert(Instant::now());
                                }
                            }

                            let mut addrs = Vec::new();

                            for (k,v) in connections.iter() {
                                let passed = v.elapsed();

                                if passed > MAX_TIME_DIFF {
                                    del_list.push(k.clone());
                                } else if k != &addr {
                                    addrs.push(k.clone());
                                }
                            }

                            while let Some(k) = del_list.pop() {
                                connections.remove(&k);
                            }

                            out_tx.send((addrs, data)).await.unwrap();
                        }
                    }
                }
            }
        }
    });

    let send = tokio::spawn(async move {
        loop {
            tokio::select! {
                Ok(_) = exit_rx1.recv() => {
                    out_rx.close();
                },
                res = out_rx.recv() => {
                    match res {
                        None => {
                            break;
                        },
                        Some((addrs, data)) => {
                            for addr in addrs.iter() {
                                udp_tx.send_to(data.as_slice(), addr).await.unwrap();
                            }
                        }
                    }
                }
            }
        }
    });

    signal::ctrl_c().await?; 

    exit_tx.send(()).unwrap();

    rec.await?;
    timeouts.await?;
    send.await?;

    Ok(())
}
