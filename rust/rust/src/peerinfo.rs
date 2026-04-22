use godot::prelude::*;
use godot::classes::{Node};
use godot::builtin::{GString};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::fs;
use crate::torrent::peer::{url_encode_bytes, extract_peers_bytes, request_piece, generate_peer_id};
use crate::torrent::utils::{decode_bencoded_value, get_i64, extract_info_hash, extract_pieces_bytes};
use reqwest;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct TPeer {
    base: Base<Node>,
    // Map URL -> List of IPs
    peers_map: Arc<Mutex<HashMap<String, Vec<String>>>>,
    // ID persistente para toda la sesión
    peer_id: Vec<u8>,
}

#[godot_api]
impl INode for TPeer {
    fn init(base: Base<Node>) -> Self {
        godot_print!("TPeer init");
        Self {
            base,
            peers_map: Arc::new(Mutex::new(HashMap::new())),
            peer_id: generate_peer_id(),
        }
    }
}

#[godot_api]
impl TPeer {

    #[signal]
    fn ips_actualizadas(url: GString, ips: GString);

    // Get the current peer id as a hex string
    #[func]
    pub fn get_my_peer_id(&self) -> GString {
        GString::from(&hex::encode(&self.peer_id))
    }

    // Implementation requesting HTTP tracker
    #[func]
    pub fn add_http_tracker(&self, url: GString, info_hash: GString) {
        let url_str = url.to_string();
        let hash_str = info_hash.to_string();
        let map_clone = self.peers_map.clone();
        let my_id = self.peer_id.clone();

        thread::spawn(move || {
            let info_hash_bytes = match hex::decode(&hash_str) {
                Ok(b) => b,
                Err(e) => {
                    println!("Error decoding hash: {}", e);
                    return;
                }
            };
            let encoded_info_hash = url_encode_bytes(&info_hash_bytes);
            let encoded_peer_id = url_encode_bytes(&my_id);

            loop {
                // Construct URL
                let tracker_url = format!(
                    "{}?info_hash={}&peer_id={}&port=6881&uploaded=0&downloaded=0&left=100&compact=1",
                    url_str,
                    encoded_info_hash,
                    encoded_peer_id
                );

                println!("Consulting tracker: {}", tracker_url);

                match reqwest::blocking::get(&tracker_url) {
                    Ok(resp) => {
                        if let Ok(bytes) = resp.bytes() {
                            // Decode to find interval
                            let decoded = decode_bencoded_value(&bytes);
                            let mut interval = get_i64(&decoded, "interval");
                            let min_interval = get_i64(&decoded, "min interval");
                            
                            if min_interval > 0 {
                                interval = min_interval;
                            }
                            if interval <= 0 {
                                interval = 1800; // Default 30 mins
                            }

                             // Extract peers
                              let peers_result = std::panic::catch_unwind(|| {
                                 extract_peers_bytes(&bytes, "peers")
                             });

                             if let Ok(peer_bytes) = peers_result {
                                 let mut ip_list = Vec::new();
                                 for chunk in peer_bytes.chunks(6) {
                                     if chunk.len() == 6 {
                                         let ip = format!("{}.{}.{}.{}", chunk[0], chunk[1], chunk[2], chunk[3]);
                                         let port = u16::from_be_bytes([chunk[4], chunk[5]]);
                                         let full_addr = format!("{}:{}", ip, port);
                                         ip_list.push(full_addr);
                                     }
                                 }

                                 // Also check for IPv6 peers
                                 if let Ok(peer_bytes_v6) = std::panic::catch_unwind(|| extract_peers_bytes(&bytes, "peers6")) {
                                     for chunk in peer_bytes_v6.chunks(18) {
                                         if chunk.len() == 18 {
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
                                             ip_list.push(format!("[{}]:{}", ip, port));
                                         }
                                     }
                                 }

                                 // Update map
                                 let mut map = map_clone.lock().unwrap();
                                 map.insert(url_str.clone(), ip_list.clone());
                                 println!("Updated {} IPs for {}", ip_list.len(), url_str);
                             } else {
                                 println!("Failed to extract peers from response");
                             }

                            println!("Sleeping for {} seconds", interval);
                            thread::sleep(Duration::from_secs(interval as u64));
                        } else {
                            println!("Failed to get bytes from response");
                            thread::sleep(Duration::from_secs(60));
                        }
                    },
                    Err(e) => {
                        println!("Request failed: {}", e);
                        thread::sleep(Duration::from_secs(60));
                    }
                }
            }
        });
    }

    #[func]
    fn get_ips(&self, url: GString) -> GString {
        let url_str = url.to_string();
        let map = self.peers_map.lock().unwrap();
        
        if let Some(ips) = map.get(&url_str) {
             let joined = ips.join(", ");
             GString::from(&joined)
        } else {
            GString::from("")
        }
    }
    
    #[func]
    fn get_all_trackers_info(&self) -> GString {
        let map = self.peers_map.lock().unwrap();
        let mut result = String::new();
        for (url, ips) in map.iter() {
            result.push_str(&format!("URL: {}\nIPs: {}\n\n", url, ips.join(", ")));
        }
        GString::from(&result)
    }

    /// Requests a piece directly without a torrent file.
    /// @param info_hash: The 20-byte info hash (hex string)
    /// @param peer_ip: The IP address (v4 or v6)
    /// @param peer_port: The port
    /// @param piece_index: The index of the piece
    /// @param piece_length: Standard length of a piece in this torrent
    /// @param total_length: Total size of the file/torrent
    /// @param expected_hash: The 20-byte SHA1 hash for this specific piece
    #[func]
    pub fn request_piece_raw(
        &self, 
        info_hash: GString, 
        peer_ip: GString, 
        peer_port: i32,
        piece_index: i32, 
        piece_length: i64, 
        total_length: i64, 
        expected_hash: PackedByteArray
    ) -> PackedByteArray {
        let hash_bytes = match hex::decode(info_hash.to_string()) {
            Ok(h) => h,
            Err(_) => {
                godot_error!("Invalid info hash hex string");
                return PackedByteArray::new();
            }
        };

        if hash_bytes.len() != 20 {
            godot_error!("Info hash must be 20 bytes");
            return PackedByteArray::new();
        }

        if expected_hash.len() != 20 {
            godot_error!("Expected hash must be 20 bytes");
            return PackedByteArray::new();
        }

        let addr = format!("{}:{}", peer_ip.to_string(), peer_port);

        match request_piece(
            &addr,
            piece_index as u32,
            piece_length as usize,
            total_length as usize,
            &hash_bytes,
            &self.peer_id,
            expected_hash.as_slice()
        ) {
            Ok(data) => {
                PackedByteArray::from_iter(data.into_iter())
            }
            Err(e) => {
                godot_error!("request_piece_raw failed for {}: {}", addr, e);
                PackedByteArray::new()
            }
        }
    }

    /// Helper to request piece using a torrent file (legacy)
    #[func]
    pub fn request_piece_from_file(&self, torrent_path: GString, piece_index: i32, peer_addr: GString) -> PackedByteArray {
        let torrent_str = torrent_path.to_string();
        let peer_str = peer_addr.to_string();
        
        let torrent_bytes = match fs::read(&torrent_str) {
            Ok(b) => b,
            Err(e) => {
                godot_error!("Failed to read torrent file: {}", e);
                return PackedByteArray::new();
            }
        };

        let decoded = decode_bencoded_value(&torrent_bytes);
        let info = match decoded.get("info") {
            Some(i) => i,
            None => return PackedByteArray::new(),
        };

        let piece_length = get_i64(info, "piece length") as usize;
        let total_length = get_i64(info, "length") as usize;
        let info_hash = hex::decode(extract_info_hash(&torrent_bytes)).unwrap_or_default();
        let piece_hashes = extract_pieces_bytes(&torrent_bytes);
        let p_idx = piece_index as usize;
        let expected_hash = &piece_hashes[p_idx * 20..p_idx * 20 + 20];

        match request_piece(
            &peer_str,
            piece_index as u32,
            piece_length,
            total_length,
            &info_hash,
            &self.peer_id,
            expected_hash
        ) {
            Ok(data) => PackedByteArray::from_iter(data.into_iter()),
            Err(e) => {
                godot_error!("request_piece failed: {}", e);
                PackedByteArray::new()
            }
        }
    }
}