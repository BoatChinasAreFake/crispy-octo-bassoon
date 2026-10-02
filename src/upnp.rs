//! Minimal UPnP IGD client: asks the home router to forward a TCP port so
//! friends can reach a player-hosted world over the internet. std-only:
//! SSDP discovery over UDP multicast, HTTP/SOAP over plain TCP.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

const SEARCH_TARGETS: [&str; 4] = [
    "urn:schemas-upnp-org:device:InternetGatewayDevice:1",
    "urn:schemas-upnp-org:device:InternetGatewayDevice:2",
    "urn:schemas-upnp-org:service:WANIPConnection:1",
    "urn:schemas-upnp-org:service:WANPPPConnection:1",
];

#[derive(Clone, Debug)]
pub struct Mapping {
    control_url: String,
    service: String,
    pub port: u16,
    /// The router's public address, if it told us.
    pub external_ip: Option<String>,
}

impl Mapping {
    /// External IPs that can't actually be reached from the internet
    /// (the router itself sits behind another NAT, e.g. carrier-grade NAT).
    pub fn behind_second_nat(&self) -> bool {
        let Some(ip) = &self.external_ip else { return false };
        let Ok(ip) = ip.parse::<std::net::Ipv4Addr>() else { return false };
        let o = ip.octets();
        ip.is_private() || (o[0] == 100 && (64..128).contains(&o[1])) || ip.is_unspecified()
    }
}

/// Find the router and forward `port` to this machine. Takes a few seconds; run it off the main thread.
pub fn open_port(port: u16) -> Result<Mapping, String> {
    let locations = discover(Duration::from_millis(2500))?;
    let mut last = "No UPnP router answered (UPnP may be turned off in the router settings).".to_string();
    for loc in locations {
        let router = host_of(&loc).map(|(h, _)| h).unwrap_or_default();
        let local_ip = local_ip_towards(&router).unwrap_or_else(|| "0.0.0.0".into());
        match open_port_via(&loc, port, &local_ip) {
            Ok(m) => return Ok(m),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Same as `open_port`, with a known device description URL.
pub fn open_port_via(location: &str, port: u16, local_ip: &str) -> Result<Mapping, String> {
    let (status, desc) = http(location, "GET", &[], "")?;
    if status != 200 {
        return Err(format!("Router description request failed (HTTP {status})."));
    }
    let (service, control) = find_service(&desc).ok_or("The router doesn't offer port mapping.")?;
    let control_url = resolve_url(location, &desc, &control);
    let mut m = Mapping { control_url, service, port, external_ip: None };

    let args = |lease: u32| {
        format!(
            "<NewRemoteHost></NewRemoteHost><NewExternalPort>{port}</NewExternalPort><NewProtocol>TCP</NewProtocol>\
             <NewInternalPort>{port}</NewInternalPort><NewInternalClient>{local_ip}</NewInternalClient>\
             <NewEnabled>1</NewEnabled><NewPortMappingDescription>Minceraft</NewPortMappingDescription>\
             <NewLeaseDuration>{lease}</NewLeaseDuration>"
        )
    };
    // Some routers refuse permanent (0) leases; fall back to a long one.
    let (status, body) = soap(&m, "AddPortMapping", &args(0))?;
    if status != 200 {
        let (status2, body2) = soap(&m, "AddPortMapping", &args(86400))?;
        if status2 != 200 {
            let why = xml_text(&body2, "errorDescription").or_else(|| xml_text(&body, "errorDescription")).unwrap_or_else(|| format!("HTTP {status2}"));
            return Err(format!("The router refused to open port {port}: {why}."));
        }
    }
    if let Ok((200, body)) = soap(&m, "GetExternalIPAddress", "") {
        m.external_ip = xml_text(&body, "NewExternalIPAddress").filter(|s| !s.is_empty());
    }
    Ok(m)
}

/// Remove the forwarding again (best effort).
pub fn close(m: &Mapping) {
    let args = format!("<NewRemoteHost></NewRemoteHost><NewExternalPort>{}</NewExternalPort><NewProtocol>TCP</NewProtocol>", m.port);
    let _ = soap(m, "DeletePortMapping", &args);
}

fn discover(wait: Duration) -> Result<Vec<String>, String> {
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Couldn't open a UDP socket: {e}"))?;
    sock.set_read_timeout(Some(Duration::from_millis(250))).ok();
    for st in SEARCH_TARGETS {
        let req = format!("M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {st}\r\n\r\n");
        let _ = sock.send_to(req.as_bytes(), "239.255.255.250:1900");
    }
    let mut found: Vec<String> = Vec::new();
    let start = Instant::now();
    let mut buf = [0u8; 2048];
    while start.elapsed() < wait {
        if let Ok((n, _)) = sock.recv_from(&mut buf) {
            let text = String::from_utf8_lossy(&buf[..n]);
            if let Some(loc) = header(&text, "location")
                && !found.contains(&loc) {
                    found.push(loc);
                }
        }
    }
    if found.is_empty() {
        Err("No UPnP router answered (UPnP may be turned off in the router settings).".into())
    } else {
        Ok(found)
    }
}

fn header(resp: &str, name: &str) -> Option<String> {
    resp.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim().to_string())
    })
}

/// (host, port, path) of an http:// URL.
fn split_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("http://")?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].to_string()),
        None => (rest, "/".to_string()),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') || h.ends_with(']') => (h.to_string(), p.parse().ok()?),
        _ => (hostport.to_string(), 80),
    };
    Some((host, port, path))
}

fn host_of(url: &str) -> Option<(String, u16)> {
    split_url(url).map(|(h, p, _)| (h, p))
}

fn local_ip_towards(host: &str) -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect((host.trim_matches(|c| c == '[' || c == ']'), 9)).ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

fn resolve_url(location: &str, desc: &str, control: &str) -> String {
    if control.starts_with("http://") {
        return control.to_string();
    }
    let base = xml_text(desc, "URLBase").filter(|b| b.starts_with("http://")).unwrap_or_else(|| {
        let (h, p) = host_of(location).unwrap_or_default();
        format!("http://{h}:{p}")
    });
    let base = base.trim_end_matches('/');
    if control.starts_with('/') { format!("{base}{control}") } else { format!("{base}/{control}") }
}

/// First WAN IP/PPP connection service: (serviceType, controlURL).
fn find_service(desc: &str) -> Option<(String, String)> {
    let mut rest = desc;
    while let Some(i) = rest.find("<service>") {
        let block_end = rest[i..].find("</service>").map(|j| i + j).unwrap_or(rest.len());
        let block = &rest[i..block_end];
        if let (Some(t), Some(c)) = (xml_text(block, "serviceType"), xml_text(block, "controlURL"))
            && (t.contains("WANIPConnection") || t.contains("WANPPPConnection")) {
                return Some((t, c));
            }
        rest = &rest[block_end..];
        if rest.len() <= "</service>".len() {
            break;
        }
        rest = &rest["</service>".len()..];
    }
    None
}

/// Text of the first `<tag>` (ignoring any namespace prefix on the tag).
fn xml_text(xml: &str, tag: &str) -> Option<String> {
    let mut search = 0;
    while let Some(i) = xml[search..].find(tag).map(|i| i + search) {
        let before = xml[..i].rfind('<')?;
        let prefix = &xml[before + 1..i];
        let after = &xml[i + tag.len()..];
        if !prefix.starts_with('/') && prefix.chars().all(|c| c.is_alphanumeric() || c == ':') && (after.starts_with('>') || after.starts_with(' ')) {
            let open_end = i + after.find('>')? + tag.len() + 1;
            let close = xml[open_end..].find("</")? + open_end;
            return Some(xml[open_end..close].trim().to_string());
        }
        search = i + tag.len();
    }
    None
}

fn soap(m: &Mapping, action: &str, args: &str) -> Result<(u16, String), String> {
    let body = format!(
        "<?xml version=\"1.0\"?>\r\n<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
         s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{svc}\">{args}</u:{action}></s:Body></s:Envelope>",
        svc = m.service
    );
    let soap_action = format!("\"{}#{}\"", m.service, action);
    http(&m.control_url, "POST", &[("Content-Type", "text/xml; charset=\"utf-8\""), ("SOAPAction", &soap_action)], &body)
}

/// Tiny HTTP/1.1 client (Connection: close, handles chunked bodies).
fn http(url: &str, method: &str, headers: &[(&str, &str)], body: &str) -> Result<(u16, String), String> {
    let (host, port, path) = split_url(url).ok_or_else(|| format!("Unsupported router URL: {url}"))?;
    let addr = (host.trim_matches(|c| c == '[' || c == ']'), port)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or("Router address didn't resolve")?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(3)).map_err(|e| format!("Couldn't reach the router: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(4))).ok();
    s.set_write_timeout(Some(Duration::from_secs(4))).ok();
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    let _ = s.read_to_end(&mut raw); // timeouts after a complete response are fine
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").ok_or("Garbled reply from the router")?;
    let status: u16 = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or("Garbled reply from the router")?;
    let chunked = header(head, "transfer-encoding").map(|v| v.eq_ignore_ascii_case("chunked")).unwrap_or(false);
    Ok((status, if chunked { dechunk(body) } else { body.to_string() }))
}

fn dechunk(mut s: &str) -> String {
    let mut out = String::new();
    while let Some((len, rest)) = s.split_once("\r\n") {
        let Ok(n) = usize::from_str_radix(len.trim().split(';').next().unwrap_or(""), 16) else { break };
        if n == 0 || rest.len() < n {
            out.push_str(&rest[..n.min(rest.len())]);
            break;
        }
        out.push_str(&rest[..n]);
        s = rest[n..].trim_start_matches("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// A pretend router that speaks just enough UPnP.
    fn fake_router() -> (String, Arc<Mutex<Vec<String>>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let log2 = log.clone();
        std::thread::spawn(move || {
            for stream in l.incoming() {
                let Ok(mut s) = stream else { continue };
                s.set_read_timeout(Some(Duration::from_millis(300))).ok();
                let mut buf = vec![0u8; 8192];
                let mut req = Vec::new();
                while let Ok(n) = s.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    req.extend_from_slice(&buf[..n]);
                    let t = String::from_utf8_lossy(&req);
                    if let Some((h, b)) = t.split_once("\r\n\r\n") {
                        let len: usize = header(h, "content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
                        if b.len() >= len {
                            break;
                        }
                    }
                }
                let req = String::from_utf8_lossy(&req).into_owned();
                log2.lock().unwrap().push(req.clone());
                let body = if req.starts_with("GET /desc.xml") {
                    // Chunked, with the service nested like real routers do.
                    let xml = "<root><device><serviceList><service><serviceType>urn:schemas-upnp-org:service:Layer3Forwarding:1</serviceType>\
                               <controlURL>/l3</controlURL></service></serviceList><deviceList><device><serviceList><service>\
                               <serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/ctl/ip</controlURL>\
                               </service></serviceList></device></deviceList></device></root>";
                    let half = xml.len() / 2;
                    let chunked = format!("{:x}\r\n{}\r\n{:x}\r\n{}\r\n0\r\n\r\n", half, &xml[..half], xml.len() - half, &xml[half..]);
                    let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{chunked}").as_bytes());
                    continue;
                } else if req.contains("#AddPortMapping") && req.contains("<NewLeaseDuration>0<") {
                    let _ = s.write_all(b"HTTP/1.1 500 Internal Server Error\r\n\r\n<errorDescription>OnlyPermanentLeasesSupported</errorDescription>");
                    continue;
                } else if req.contains("#GetExternalIPAddress") {
                    "<s:Envelope><s:Body><u:GetExternalIPAddressResponse><NewExternalIPAddress>203.0.113.7</NewExternalIPAddress></u:GetExternalIPAddressResponse></s:Body></s:Envelope>"
                } else {
                    "<s:Envelope><s:Body/></s:Envelope>"
                };
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes());
            }
        });
        (format!("http://127.0.0.1:{port}/desc.xml"), log)
    }

    #[test]
    fn maps_a_port_through_a_router() {
        let (loc, log) = fake_router();
        let m = open_port_via(&loc, 25565, "192.168.1.50").expect("mapping should succeed");
        assert_eq!(m.external_ip.as_deref(), Some("203.0.113.7"));
        assert!(!m.behind_second_nat());
        assert!(m.control_url.ends_with("/ctl/ip"), "{}", m.control_url);
        close(&m);
        let log = log.lock().unwrap();
        let adds: Vec<&String> = log.iter().filter(|r| r.contains("#AddPortMapping")).collect();
        assert_eq!(adds.len(), 2, "should retry with a finite lease");
        assert!(adds[1].contains("<NewExternalPort>25565</NewExternalPort>") && adds[1].contains("<NewInternalClient>192.168.1.50<"));
        assert!(log.iter().any(|r| r.contains("#DeletePortMapping")));
    }

    #[test]
    fn helpers() {
        assert_eq!(split_url("http://192.168.1.1:5000/rootDesc.xml"), Some(("192.168.1.1".into(), 5000, "/rootDesc.xml".into())));
        assert_eq!(split_url("http://router/x"), Some(("router".into(), 80, "/x".into())));
        assert_eq!(xml_text("<a><m:NewExternalIPAddress>1.2.3.4</m:NewExternalIPAddress></a>", "NewExternalIPAddress").as_deref(), Some("1.2.3.4"));
        assert_eq!(header("HTTP/1.1 200 OK\r\nLOCATION: http://x/y\r\n", "location").as_deref(), Some("http://x/y"));
        let cg = Mapping { control_url: String::new(), service: String::new(), port: 1, external_ip: Some("100.72.1.2".into()) };
        assert!(cg.behind_second_nat());
    }
}
