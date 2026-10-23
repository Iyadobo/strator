use crate::catalog::LaunchSpec;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

pub struct Handoff {
    pub address: String,
    pub token: String,
    pub claimed: Receiver<bool>,
}
pub fn serve(spec: LaunchSpec) -> std::io::Result<Handoff> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let mut entropy = [0u8; 32];
    getrandom::fill(&mut entropy).map_err(|error| std::io::Error::other(error.to_string()))?;
    let token: String = entropy.iter().map(|b| format!("{b:02x}")).collect();
    let expected = token.clone();
    let address = listener.local_addr()?.to_string();
    let (tx, claimed) = mpsc::channel();
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(45);
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                    let mut received = String::new();
                    if (&mut stream)
                        .take(129)
                        .read_to_string(&mut received)
                        .is_err()
                    {
                        continue;
                    }
                    if received.trim() != expected {
                        continue;
                    }
                    let sent = serde_json::to_writer(&mut stream, &spec).is_ok();
                    let _ = stream.shutdown(Shutdown::Write);
                    let _ = tx.send(sent);
                    return;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(false);
    });
    Ok(Handoff {
        address,
        token,
        claimed,
    })
}
pub fn receive(address: &str, token: &str) -> std::io::Result<LaunchSpec> {
    let addr: std::net::SocketAddr = address.parse().map_err(std::io::Error::other)?;
    if !addr.ip().is_loopback() {
        return Err(std::io::Error::other(
            "Credential handoff must stay on loopback.",
        ));
    }
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(token.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;
    let mut payload = Vec::new();
    stream.take(128 * 1024).read_to_end(&mut payload)?;
    serde_json::from_slice(&payload)
        .map_err(|_| std::io::Error::other("Launch ticket rejected or expired."))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ticket_is_authenticated_and_single_use() {
        let mut env = std::collections::BTreeMap::new();
        env.insert("TEST_API_KEY".into(), "fake-test-key".into());
        let h = serve(LaunchSpec {
            command: "test".into(),
            args: vec![],
            env,
            cwd: ".".into(),
        })
        .unwrap();
        assert!(receive(&h.address, "wrong-ticket").is_err());
        let received = receive(&h.address, &h.token).unwrap();
        assert_eq!(received.env["TEST_API_KEY"], "fake-test-key");
        assert!(h.claimed.recv_timeout(Duration::from_secs(1)).unwrap());
        assert!(receive(&h.address, &h.token).is_err());
    }
    #[test]
    fn external_handoff_is_rejected() {
        assert!(receive("8.8.8.8:1234", "fake").is_err());
    }
}
