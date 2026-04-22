use reqwest;
use super::utils::{decode_bencoded_value_with_index};
use super::utils::{decode_bencoded_value, extract_info_hash, extract_pieces_bytes, get_str, get_i64};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use sha1::{Sha1, Digest};
use crate::state::{GLOBAL_IPS, GLOBAL_HTTP};



pub fn print_peers(bytes: &[u8]) {
   // let bytes = fs::read(torrent_path).expect("Failed to read file");



   // let mut file = OpenOptions::new()
     //   .read(true)
     //   .open(torrent_path)
      //  .expect("No se pudo abrir el archivo para lectura");

   // let mut bytes = Vec::new();
   // file.read_to_end(&mut bytes)
      //  .expect("No se pudo leer el archivo");


    let decoded = decode_bencoded_value(&bytes);
    let announce = get_str(&decoded, "announce");
    let length = decoded
        .get("info")
        .map(|info| get_i64(info, "length"))
        .unwrap_or(0);

    let info_hash_hex = extract_info_hash(&bytes);
    let info_hash_bytes = hex::decode(info_hash_hex).expect("Invalid info hash");

    let encoded_info_hash = url_encode_bytes(&info_hash_bytes);

    let rand_str: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();
    let peer_id = format!("-AY0001-{}", rand_str);

    let tracker_url = format!(
        "{}?info_hash={}&peer_id={}&port=6881&uploaded=0&downloaded=0&left={}&compact=1",
        announce,
        encoded_info_hash,
        peer_id,
        length
    );
    GLOBAL_HTTP.lock().unwrap().push(format!("{}  {}", tracker_url, announce));
    let response = reqwest::blocking::get(&tracker_url)
        .expect("Tracker request failed")
        .bytes()
        .expect("Failed to read response");

    let peer_bytes_v4 = extract_peers_bytes(&response, "peers");
    let peer_bytes_v6 = extract_peers_bytes(&response, "peers6");

    // Process IPv4 peers
    for chunk in peer_bytes_v4.chunks(6) {
        if chunk.len() < 6 {
            continue;
        }

        let ip = format!("{}.{}.{}.{}", chunk[0], chunk[1], chunk[2], chunk[3]);
        let port = u16::from_be_bytes([chunk[4], chunk[5]]);
        let addr = format!("{}:{}", ip, port);
        GLOBAL_IPS.lock().unwrap().push(addr.clone());
        println!("{}", addr);
    }

    // Process IPv6 peers
    for chunk in peer_bytes_v6.chunks(18) {
        if chunk.len() < 18 {
            continue;
        }

        let ip = std::net::Ipv6Addr::new(
            u16::from_be_bytes([chunk[0], chunk[1]]),
            u16::from_be_bytes([chunk[2], chunk[3]]),
            u16::from_be_bytes([chunk[4], chunk[5]]),
            u16::from_be_bytes([chunk[6], chunk[7]]),
            u16::from_be_bytes([chunk[8], chunk[9]]),
            u16::from_be_bytes([chunk[10], chunk[11]]),
            u16::from_be_bytes([chunk[12], chunk[13]]),
            u16::from_be_bytes([chunk[14], chunk[15]]),
        );
        let port = u16::from_be_bytes([chunk[16], chunk[17]]);
        let addr = format!("[{}]:{}", ip, port);
        GLOBAL_IPS.lock().unwrap().push(addr.clone());
        println!("{}", addr);
    }
}

pub fn url_encode_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("%{:02X}", b)).collect()
}


// extract the raw peer list (in compact format) from the bencoded tracker response
pub fn extract_peers_bytes(bytes: &[u8], target_key: &str) -> Vec<u8> {
    let mut index = 0;
    if bytes.is_empty() || bytes[0] as char != 'd' {
        return Vec::new();
    }
    index += 1;

    let target_key_bytes = target_key.as_bytes();

    while index < bytes.len() && bytes[index] as char != 'e' {
        // Parse key
        let key_start = index;
        while index < bytes.len() && bytes[index] as char != ':' {
            index += 1;
        }

        if index >= bytes.len() { break; }

        let key_len = std::str::from_utf8(&bytes[key_start..index])
            .unwrap_or("0")
            .parse::<usize>()
            .unwrap_or(0);
        index += 1;

        if index + key_len > bytes.len() { break; }
        let key = &bytes[index..index + key_len];
        index += key_len;

        if key == target_key_bytes {
            let len_start = index;
            while index < bytes.len() && bytes[index] as char != ':' {
                index += 1;
            }

            if index >= bytes.len() { break; }

            let len_str = std::str::from_utf8(&bytes[len_start..index]).unwrap_or("0");
            let len = len_str.parse::<usize>().unwrap_or(0);
            index += 1;

            if index + len > bytes.len() { break; }
            return bytes[index..index + len].to_vec();
        } else {
            decode_bencoded_value_with_index(bytes, &mut index); 
        }
    }

    Vec::new()
}


pub fn get_first_peer(
    announce: &str, 
    info_hash: &[u8], 
    peer_id: &[u8], 
    length: usize
) -> String {
    let encoded_info_hash = info_hash.iter().map(|b| format!("%{:02X}", b)).collect::<String>();
    let encoded_peer_id = peer_id.iter().map(|b| format!("%{:02X}", b)).collect::<String>();

    let url = format!(
        "{}?info_hash={}&peer_id={}&port=6881&uploaded=0&downloaded=0&left={}&compact=1",
        announce, encoded_info_hash, encoded_peer_id, length
    );

    let response = reqwest::blocking::get(&url).unwrap().bytes().unwrap();
    let peers = extract_peers_bytes(&response, "peers");

    if peers.len() < 6 {
        // Try IPv6
        let peers6 = extract_peers_bytes(&response, "peers6");
        if peers6.len() >= 18 {
             let chunk = &peers6[0..18];
             let ip = std::net::Ipv6Addr::new(
                 u16::from_be_bytes([chunk[0], chunk[1]]),
                 u16::from_be_bytes([chunk[2], chunk[3]]),
                 u16::from_be_bytes([chunk[4], chunk[5]]),
                 u16::from_be_bytes([chunk[6], chunk[7]]),
                 u16::from_be_bytes([chunk[8], chunk[9]]),
                 u16::from_be_bytes([chunk[10], chunk[11]]),
                 u16::from_be_bytes([chunk[12], chunk[13]]),
                 u16::from_be_bytes([chunk[14], chunk[15]]),
             );
             let port = u16::from_be_bytes([chunk[16], chunk[17]]);
             return format!("[{}]:{}", ip, port);
        }
        panic!("No peers found");
    }

    let chunk = &peers[0..6];
    let ip = format!("{}.{}.{}.{}", chunk[0], chunk[1], chunk[2], chunk[3]);
    let port = u16::from_be_bytes([chunk[4], chunk[5]]);
    format!("{}:{}", ip, port)
}
/// Generates a peer_id and its encoded form.
pub fn generate_peer_id() -> Vec<u8> {
    thread_rng()
        .sample_iter::<u8, _>(rand::distributions::Standard)
        .take(20)
        .collect()
}

/// Requests a piece from a peer. 
/// Based on logic from download.rs but specialized for peer interaction.
pub fn request_piece(
    peer_addr: &str,
    piece_index: u32,
    piece_length: usize,
    total_length: usize,
    info_hash: &[u8],
    peer_id: &[u8],
    expected_hash: &[u8],
) -> Result<Vec<u8>, String> {
    // Ensure IPv6 addresses are properly formatted with brackets if they contain multiple colons
    let formatted_addr = if peer_addr.contains(':') && !peer_addr.starts_with('[') && peer_addr.chars().filter(|&c| c == ':').count() > 1 {
        if let Some(pos) = peer_addr.rfind(':') {
            let (ip, port) = peer_addr.split_at(pos);
            format!("[{}]{}", ip, port)
        } else {
            peer_addr.to_string()
        }
    } else {
        peer_addr.to_string()
    };

    let mut stream = TcpStream::connect(&formatted_addr).map_err(|e| format!("Failed to connect to {}: {}", formatted_addr, e))?;

    // 1. Handshake
    let mut handshake = Vec::new();
    handshake.push(19);
    handshake.extend_from_slice(b"BitTorrent protocol");
    handshake.extend_from_slice(&[0u8; 8]);
    handshake.extend_from_slice(info_hash);
    handshake.extend_from_slice(peer_id);
    stream.write_all(&handshake).map_err(|e| format!("Failed to send handshake: {}", e))?;

    let mut handshake_res = [0u8; 68];
    stream.read_exact(&mut handshake_res).map_err(|e| format!("Failed to read handshake response: {}", e))?;

    // 2. Leer mensajes hasta recibir unchoke (ID=1)
    // Los peers reales pueden mandar bitfield, have, unchoke en cualquier orden
    let mut len_buf = [0u8; 4];
    let mut unchoked = false;
    
    for _ in 0..20 { // máximo 20 mensajes antes de rendirse
        stream.read_exact(&mut len_buf).map_err(|e| format!("Failed to read message length: {}", e))?;
        let msg_len = u32::from_be_bytes(len_buf);
        
        // Keep-alive (len=0), ignorar y continuar
        if msg_len == 0 {
            continue;
        }
        
        let mut msg_id = [0u8; 1];
        stream.read_exact(&mut msg_id).map_err(|e| format!("Failed to read message ID: {}", e))?;
        
        let payload_len = (msg_len - 1) as usize;
        
        match msg_id[0] {
            1 => {
                // Unchoke — lo que esperamos
                unchoked = true;
                break;
            }
            5 => {
                // Bitfield — leer y descartar
                let mut bitfield = vec![0u8; payload_len];
                stream.read_exact(&mut bitfield).map_err(|e| format!("Failed to read bitfield: {}", e))?;
                // Ahora que tenemos el bitfield, enviamos Interested
                stream.write_all(&[0, 0, 0, 1, 2]).map_err(|e| format!("Failed to send interested: {}", e))?;
            }
            4 => {
                // Have — leer y descartar
                let mut payload = vec![0u8; payload_len];
                stream.read_exact(&mut payload).map_err(|e| format!("Failed to read have: {}", e))?;
            }
            0 => {
                // Choke — el peer nos está chokando, error
                return Err("Peer sent choke, cannot download".to_string());
            }
            _ => {
                // Otro mensaje desconocido — leer y descartar
                let mut payload = vec![0u8; payload_len];
                stream.read_exact(&mut payload).map_err(|e| format!("Failed to read unknown msg {}: {}", msg_id[0], e))?;
            }
        }
    }
    
    if !unchoked {
        return Err("Never received unchoke from peer".to_string());
    }

    // 5. Download blocks
    let block_size = 16 * 1024;
    let piece_data_len = piece_length.min(total_length - (piece_index as usize * piece_length));
    let mut piece_data = vec![0u8; piece_data_len];
    let mut offset = 0;
    let mut msg_id = [0u8; 1]; // reutilizado para leer IDs de mensajes de bloque

    while offset < piece_data.len() {
        let len = block_size.min(piece_data.len() - offset);

        // Send Request
        let mut req = Vec::new();
        req.extend_from_slice(&13u32.to_be_bytes());
        req.push(6);
        req.extend_from_slice(&piece_index.to_be_bytes());
        req.extend_from_slice(&(offset as u32).to_be_bytes());
        req.extend_from_slice(&(len as u32).to_be_bytes());
        stream.write_all(&req).map_err(|e| format!("Failed to send block request: {}", e))?;

        // Receive Piece block
        stream.read_exact(&mut len_buf).map_err(|e| format!("Failed to read block length: {}", e))?;
        stream.read_exact(&mut msg_id).map_err(|e| format!("Failed to read block msg ID: {}", e))?;
        if msg_id[0] != 7 {
            return Err(format!("Expected piece block (7), got {}", msg_id[0]));
        }

        let mut header = [0u8; 8]; // index + begin
        stream.read_exact(&mut header).map_err(|e| format!("Failed to read block header: {}", e))?;
        stream.read_exact(&mut piece_data[offset..offset + len]).map_err(|e| format!("Failed to read block data: {}", e))?;
        offset += len;
    }

    // 6. Validate
    let mut hasher = Sha1::new();
    hasher.update(&piece_data);
    let hash = hasher.finalize();
    if hash[..] != expected_hash[..] {
        return Err("Piece hash mismatch!".to_string());
    }

    Ok(piece_data)
}

/// A higher-level helper to request a piece given a torrent file and piece index.
pub fn request_piece_by_index(
    peer_addr: &str,
    torrent_file: &str,
    piece_index: usize,
) -> Result<Vec<u8>, String> {
    let torrent_bytes = fs::read(torrent_file).map_err(|e| format!("Failed to read torrent file: {}", e))?;
    let decoded = decode_bencoded_value(&torrent_bytes);
    
    let info = decoded.get("info").ok_or("Missing 'info' dict")?;
    let piece_length = get_i64(info, "piece length") as usize;
    let total_length = get_i64(info, "length") as usize;
    
    let info_hash_hex = extract_info_hash(&torrent_bytes);
    let info_hash = hex::decode(info_hash_hex).map_err(|e| format!("Invalid info hash: {}", e))?;
    
    let piece_hashes = extract_pieces_bytes(&torrent_bytes);
    if piece_index * 20 + 20 > piece_hashes.len() {
        return Err(format!("Piece index {} out of range", piece_index));
    }
    let expected_hash = &piece_hashes[piece_index * 20..piece_index * 20 + 20];
    
    let peer_id = generate_peer_id();
    
    request_piece(
        peer_addr,
        piece_index as u32,
        piece_length,
        total_length,
        &info_hash,
        &peer_id,
        expected_hash
    )
}

