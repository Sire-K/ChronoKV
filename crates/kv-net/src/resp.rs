use std::io::{self, BufRead};

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Ping,
    Set { key: String, value: String },
    Get { key: String },
    Del { key: String },
}

pub fn read_command<R: BufRead>(reader: &mut R) -> io::Result<Option<Result<Command, String>>> {
    let mut line = String::new();
    let bytes_read = reader.read_line(&mut line)?;
    if bytes_read == 0 {
        return Ok(None);
    }

    let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
    if trimmed.is_empty() {
        return Ok(Some(Err("empty command".to_string())));
    }

    if let Some(rest) = trimmed.strip_prefix('*') {
        let count: usize = match rest.parse() {
            Ok(n) if n > 0 => n,
            _ => return Ok(Some(Err("invalid RESP array length".to_string()))),
        };
        read_resp_array(reader, count).map(|res| Some(res))
    } else {
        Ok(Some(parse_inline(trimmed)))
    }
}

fn read_resp_array<R: BufRead>(reader: &mut R, count: usize) -> io::Result<Result<Command, String>> {
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        let mut len_line = String::new();
        if reader.read_line(&mut len_line)? == 0 {
            return Ok(Err("unexpected EOF in RESP bulk string".to_string()));
        }
        let len_str = len_line.trim_end_matches(&['\r', '\n'][..]);
        let Some(len_part) = len_str.strip_prefix('$') else {
            return Ok(Err("expected '$' bulk string header".to_string()));
        };
        let byte_len: usize = match len_part.parse() {
            Ok(n) => n,
            Err(_) => return Ok(Err("invalid bulk string length".to_string())),
        };

        let mut buf = vec![0u8; byte_len + 2]; // include trailing \r\n
        reader.read_exact(&mut buf)?;
        if &buf[byte_len..] != b"\r\n" {
            return Ok(Err("missing CRLF after bulk string".to_string()));
        }
        let s = match String::from_utf8(buf[..byte_len].to_vec()) {
            Ok(v) => v,
            Err(_) => return Ok(Err("bulk string is not valid UTF-8".to_string())),
        };
        items.push(s);
    }

    let args: Vec<&str> = items.iter().map(String::as_str).collect();
    Ok(build_command(&args))
}

fn parse_inline(line: &str) -> Result<Command, String> {
    let mut parts = line.splitn(3, ' ');
    let first = parts.next().unwrap_or("");
    let second = parts.next();
    let third = parts.next();

    let mut args = Vec::new();
    if !first.is_empty() {
        args.push(first);
    }
    if let Some(s) = second {
        args.push(s);
    }
    if let Some(t) = third {
        args.push(t);
    }
    build_command(&args)
}

fn build_command(args: &[&str]) -> Result<Command, String> {
    let Some(cmd_raw) = args.first() else {
        return Err("empty command".to_string());
    };
    match cmd_raw.to_ascii_uppercase().as_str() {
        "PING" => {
            if args.len() != 1 {
                return Err("wrong number of arguments for 'ping'".to_string());
            }
            Ok(Command::Ping)
        }
        "SET" => {
            if args.len() != 3 {
                return Err("wrong number of arguments for 'set'".to_string());
            }
            Ok(Command::Set {
                key: args[1].to_string(),
                value: args[2].to_string(),
            })
        }
        "GET" => {
            if args.len() != 2 {
                return Err("wrong number of arguments for 'get'".to_string());
            }
            Ok(Command::Get {
                key: args[1].to_string(),
            })
        }
        "DEL" => {
            if args.len() != 2 {
                return Err("wrong number of arguments for 'del'".to_string());
            }
            Ok(Command::Del {
                key: args[1].to_string(),
            })
        }
        other => Err(format!("unknown command '{other}'")),
    }
}
