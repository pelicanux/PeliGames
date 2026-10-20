use std::{
    io::{self, BufReader, Read},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

/// Downloaders update a single terminal line with CR, without a newline.
/// Bound records too: verbose tools must not build an unbounded event payload.
#[cfg(test)]
pub fn records(pipe: impl Read, emit: impl FnMut(String)) -> io::Result<()> {
    records_until(pipe, &AtomicBool::new(false), emit)
}
pub fn records_until(
    pipe: impl Read,
    stop: &AtomicBool,
    mut emit: impl FnMut(String),
) -> io::Result<()> {
    let mut pipe = BufReader::new(pipe);
    let mut record = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(30));
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        for &byte in &buffer[..count] {
            if byte == b'\r' || byte == b'\n' {
                if !record.is_empty() {
                    emit(String::from_utf8_lossy(&record).into_owned());
                    record.clear();
                }
            } else {
                record.push(byte);
                if record.len() >= 16_384 {
                    emit(String::from_utf8_lossy(&record).into_owned());
                    record.clear();
                }
            }
        }
        if stop.load(Ordering::Acquire) {
            break;
        }
    }
    if !record.is_empty() {
        emit(String::from_utf8_lossy(&record).into_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streams_download_updates_and_final_partial_line() {
        let mut lines = vec![];
        records(&b"10%\r20%\r\nInstalling\nDone"[..], |line| {
            lines.push(line)
        })
        .unwrap();
        assert_eq!(lines, ["10%", "20%", "Installing", "Done"]);
    }
    #[test]
    fn bounds_long_records_and_keeps_unicode() {
        let mut lines = vec![];
        records("ação\r".as_bytes(), |line| lines.push(line)).unwrap();
        assert_eq!(lines, ["ação"]);
        lines.clear();
        records(vec![b'x'; 40_000].as_slice(), |line| lines.push(line)).unwrap();
        assert_eq!(lines.iter().map(String::len).sum::<usize>(), 40_000);
        assert!(lines.iter().all(|line| line.len() <= 16_384));
    }
}
