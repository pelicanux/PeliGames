//! Read-only OpenRGB SDK v1 client. New servers serialize the negotiated v1 layout.
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    time::{Duration, Instant},
};
const LIMIT: usize = 2 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub name: String,
    pub color: Option<[u8; 3]>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub key: String,
    pub name: String,
    pub zones: Vec<Zone>,
    pub color: Option<[u8; 3]>,
}
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).ok_or("Tamanho inválido")?;
        let data = self
            .data
            .get(self.pos..end)
            .ok_or("Resposta OpenRGB incompleta")?;
        self.pos = end;
        Ok(data)
    }
    fn u16(&mut self) -> Result<usize, String> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()) as usize)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String, String> {
        let n = self.u16()?;
        let data = self.bytes(n)?;
        if data.last() != Some(&0) {
            return Err("Texto OpenRGB inválido".into());
        }
        Ok(String::from_utf8_lossy(&data[..n - 1])
            .chars()
            .filter(|c| !c.is_control())
            .take(256)
            .collect())
    }
    fn colors(&mut self) -> Result<Vec<[u8; 3]>, String> {
        let n = self.u16()?;
        let mut colors = Vec::with_capacity(n);
        for _ in 0..n {
            let c = self.bytes(4)?;
            colors.push([c[0], c[1], c[2]]);
        }
        Ok(colors)
    }
}
fn average(colors: &[[u8; 3]]) -> Option<[u8; 3]> {
    if colors.is_empty() {
        return None;
    }
    let mut sum = [0u64; 3];
    for c in colors {
        for i in 0..3 {
            sum[i] += c[i] as u64;
        }
    }
    Some(sum.map(|v| (v / colors.len() as u64) as u8))
}
pub fn parse_device(data: &[u8]) -> Result<Device, String> {
    let mut c = Cursor { data, pos: 0 };
    if c.u32()? as usize != data.len() {
        return Err("Tamanho do dispositivo OpenRGB inválido".into());
    }
    c.u32()?;
    let name = c.string()?;
    let vendor = c.string()?;
    c.string()?;
    c.string()?;
    let serial = c.string()?;
    let location = c.string()?;
    let modes = c.u16()?;
    let active = c.u32()? as usize;
    let mut mode_color = None;
    for mode in 0..modes {
        c.string()?;
        c.bytes(9 * 4)?;
        let colors = c.colors()?;
        if mode == active {
            mode_color = average(&colors);
        }
    }
    let n = c.u16()?;
    let mut zone_ranges = Vec::with_capacity(n);
    let mut offset = 0usize;
    for _ in 0..n {
        let name = c.string()?;
        c.bytes(12)?;
        let count = c.u32()? as usize;
        let matrix = c.u16()?;
        c.bytes(matrix)?;
        zone_ranges.push((name, offset, count));
        offset = offset.checked_add(count).ok_or("Zona inválida")?;
    }
    let leds = c.u16()?;
    for _ in 0..leds {
        c.string()?;
        c.u32()?;
    }
    let colors = c.colors()?;
    if c.pos != data.len() {
        return Err("Layout OpenRGB inesperado".into());
    }
    let zones = zone_ranges
        .into_iter()
        .map(|(name, start, count)| Zone {
            name,
            color: start
                .checked_add(count)
                .and_then(|end| colors.get(start..end))
                .and_then(average),
        })
        .collect();
    Ok(Device {
        key: format!("{vendor}|{name}|{serial}|{location}"),
        name: if vendor.is_empty() || name.starts_with(&vendor) {
            name
        } else {
            format!("{vendor} {name}")
        },
        zones,
        color: average(&colors).or(mode_color),
    })
}
pub struct Client {
    stream: TcpStream,
}
impl Client {
    fn send(&mut self, device: u32, id: u32, body: &[u8]) -> Result<(), String> {
        let mut header = Vec::from(*b"ORGB");
        header.extend(device.to_le_bytes());
        header.extend(id.to_le_bytes());
        header.extend((body.len() as u32).to_le_bytes());
        self.stream
            .write_all(&header)
            .and_then(|_| self.stream.write_all(body))
            .map_err(|_| "OpenRGB desconectado".into())
    }
    fn response(&mut self, device: u32, id: u32) -> Result<Vec<u8>, String> {
        let until = Instant::now() + Duration::from_secs(2);
        for _ in 0..16 {
            if Instant::now() > until {
                return Err("O OpenRGB demorou para responder".into());
            }
            let mut header = [0; 16];
            self.stream
                .read_exact(&mut header)
                .map_err(|_| "Sem resposta do OpenRGB. Ative o servidor SDK.".to_string())?;
            if &header[..4] != b"ORGB" {
                return Err("Servidor não é um OpenRGB SDK".into());
            }
            let dev = u32::from_le_bytes(header[4..8].try_into().unwrap());
            let packet = u32::from_le_bytes(header[8..12].try_into().unwrap());
            let len = u32::from_le_bytes(header[12..16].try_into().unwrap()) as usize;
            if len > LIMIT {
                return Err("Resposta OpenRGB excede o limite".into());
            }
            let mut data = vec![0; len];
            self.stream
                .read_exact(&mut data)
                .map_err(|_| "Resposta OpenRGB interrompida".to_string())?;
            if packet == 100 {
                return Err("A lista do OpenRGB mudou. Reconectando…".into());
            }
            if dev == device && packet == id {
                return Ok(data);
            }
        }
        Err("Muitas notificações do OpenRGB. Tente novamente.".into())
    }
    pub fn connect(port: u16) -> Result<Self, String> {
        if port == 0 {
            return Err("Porta OpenRGB inválida".into());
        }
        let stream = TcpStream::connect_timeout(
            &SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            Duration::from_millis(700),
        )
        .map_err(|_| {
            "OpenRGB não encontrado em 127.0.0.1. Abra o OpenRGB e ative SDK Server → Start Server."
                .to_string()
        })?;
        stream
            .set_read_timeout(Some(Duration::from_millis(900)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_millis(900)))
            .map_err(|e| e.to_string())?;
        let mut client = Self { stream };
        client.send(0, 40, &1u32.to_le_bytes())?;
        let version = client.response(0, 40)?;
        if version.len() != 4 || u32::from_le_bytes(version[..].try_into().unwrap()) < 1 {
            return Err("Use OpenRGB com protocolo SDK 1 ou superior (0.5+).".into());
        }
        client.send(0, 50, b"PeliGames (read-only color sync)\0")?;
        Ok(client)
    }
    pub fn devices(&mut self) -> Result<Vec<Device>, String> {
        self.send(0, 0, &[])?;
        let response = self.response(0, 0)?;
        if response.len() != 4 {
            return Err("Contagem OpenRGB inválida".into());
        }
        let count = u32::from_le_bytes(response[..].try_into().unwrap());
        if count > 64 {
            return Err("O OpenRGB informou mais de 64 dispositivos".into());
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut devices = Vec::new();
        for index in 0..count {
            if Instant::now() > deadline {
                return Err("A leitura dos dispositivos OpenRGB demorou demais".into());
            }
            self.send(index, 1, &1u32.to_le_bytes())?;
            devices.push(parse_device(&self.response(index, 1)?)?);
        }
        Ok(devices)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn string(b: &mut Vec<u8>, s: &str) {
        b.extend(((s.len() + 1) as u16).to_le_bytes());
        b.extend(s.as_bytes());
        b.push(0);
    }
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; 8];
        for s in ["Test", "Vendor", "Description", "1", "serial", "usb:1"] {
            string(&mut b, s);
        }
        b.extend(1u16.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        string(&mut b, "Static");
        b.extend([0; 36]);
        b.extend(1u16.to_le_bytes());
        b.extend([11, 22, 33, 0]);
        b.extend(1u16.to_le_bytes());
        string(&mut b, "Front");
        b.extend([0; 12]);
        b.extend(2u32.to_le_bytes());
        b.extend(0u16.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        for n in ["LED 1", "LED 2"] {
            string(&mut b, n);
            b.extend([0; 4]);
        }
        b.extend(2u16.to_le_bytes());
        b.extend([20, 40, 60, 0, 40, 60, 80, 0]);
        let len = b.len() as u32;
        b[..4].copy_from_slice(&len.to_le_bytes());
        b
    }
    #[test]
    fn parses_protocol_one_and_zone_color() {
        let d = parse_device(&fixture()).unwrap();
        assert_eq!(d.color, Some([30, 50, 70]));
        assert_eq!(d.zones[0].color, d.color);
        assert!(d.key.contains("serial"));
    }
    #[test]
    fn rejects_every_truncation() {
        let data = fixture();
        for end in 0..data.len() {
            assert!(parse_device(&data[..end]).is_err());
        }
    }
    #[test]
    fn rejects_invalid_sizes_and_nonterminated_strings() {
        let mut data = fixture();
        data[0] = 0;
        assert!(parse_device(&data).is_err());
        let mut data = fixture();
        data[14] = 9;
        assert!(parse_device(&data).is_err());
    }
    #[test]
    fn loopback_server_only_receives_read_requests() {
        use std::net::TcpListener;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let worker = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            for expected in [40u32, 50, 0, 1] {
                let mut h = [0; 16];
                s.read_exact(&mut h).unwrap();
                let id = u32::from_le_bytes(h[8..12].try_into().unwrap());
                assert_eq!(id, expected);
                let n = u32::from_le_bytes(h[12..16].try_into().unwrap());
                let mut b = vec![0; n as usize];
                s.read_exact(&mut b).unwrap();
                let reply = match id {
                    40 => 6u32.to_le_bytes().to_vec(),
                    50 => continue,
                    0 => 1u32.to_le_bytes().to_vec(),
                    1 => {
                        assert_eq!(b, 1u32.to_le_bytes());
                        fixture()
                    }
                    _ => unreachable!(),
                };
                h[12..16].copy_from_slice(&(reply.len() as u32).to_le_bytes());
                s.write_all(&h).unwrap();
                s.write_all(&reply).unwrap();
            }
        });
        let mut c = Client::connect(port).unwrap();
        assert_eq!(c.devices().unwrap()[0].color, Some([30, 50, 70]));
        worker.join().unwrap();
    }
}
