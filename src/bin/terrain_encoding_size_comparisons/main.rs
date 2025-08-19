use std::env;
use std::mem::size_of;
use screeps::{RoomName, Terrain};

use screeps_map_processing::compressed_terrain::compressed_terrain::{CompressedRoomTerrain, COMPRESSED_ARRAY_SIZE};
use screeps_map_processing::compressed_terrain_db;
use screeps_map_processing::run_length_encoding::rle_terrain::{RoomTerrainPackedIndexedRLE, BinarySearchPackedRoomTerrainRLE, PackedRLERoomTerrain, RLERoomTerrain, WildcardRLERoomTerrain};
use screeps_map_processing::run_length_encoding::generic_rle::{BinarySearchRLE, IndexedRLE};

const VERBOSE: bool = false;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    let path_to_compressed_db_file = &args[1];

    if VERBOSE {
        println!("== Data Storage Sizes ==");
    }

    // let rooms_to_check_str = vec!(
    //     "W23S45", // Very swampy and separated, lots of runs
    //     "W20S40", // Crossroads, very open, low amount of runs
    //     "W20S41", // Highway, very open, but does have obstacles, reasonable amount of runs
    //     "W20S42", // Highway, similar to W20S41
    // );

    // let rooms_to_check = rooms_to_check_str.iter().filter_map(|name| RoomName::new(name).ok()).collect::<Vec<RoomName>>();

    if let Ok(conn) = compressed_terrain_db::open_db_file(path_to_compressed_db_file) {
        let create_table_res = compressed_terrain_db::create_terrain_table_if_not_exists(&conn);
        if create_table_res.is_ok() {
            let rooms_res = compressed_terrain_db::get_rooms_with_terrain(&conn);
            if let Ok(rooms) = rooms_res {
                // Collect some stats
                let mut rooms_processed = 0;

                let mut rle_packed_runs: Vec<usize> = Vec::new();
                let mut rle_wildcard_runs: Vec<usize> = Vec::new();

                let mut rooms_optimal_compressed: Vec<(RoomName, usize)> = Vec::new();
                let mut rooms_optimal_rle_packed: Vec<(RoomName, usize)> = Vec::new();
                let mut rooms_optimal_rle_wildcard: Vec<(RoomName, usize)> = Vec::new();

                for room_name in rooms {
                    // if !rooms_to_check.contains(&room_name) {
                    //     continue;
                    // }

                    if let Ok(compressed_terrain) = compressed_terrain_db::get_terrain_for_room(&conn, room_name) {
                        rooms_processed += 1;

                        let compressed_size = compressed_terrain.memory_size();
                        if VERBOSE {
                            println!("");
                            println!("Room {room_name:?}");

                            println!("CompressedRoomTerrain Size: {}", compressed_terrain.memory_size());
                        }

                        // Now that we have the compressed terrain, generate the RLE terrain from
                        // it
                        let rle_terrain = RLERoomTerrain::new_from_compressed_terrain(&compressed_terrain);
                        let num_runs = rle_terrain.num_runs();

                        if VERBOSE {
                            println!("RLE Terrain u16 Size: {}", rle_terrain.memory_size());
                            println!("Num Runs: {}", num_runs);
                        }

                        let rle_terrain = PackedRLERoomTerrain::new_from_compressed_terrain(&compressed_terrain);
                        let num_runs = rle_terrain.num_runs();
                        let rle_packed_size = rle_terrain.memory_size();
                        rle_packed_runs.push(num_runs);

                        if VERBOSE {
                            println!("Bit-packed RLE Terrain Size: {}", rle_terrain.memory_size());
                            println!("Num Runs: {}", num_runs);
                        }

                        let rle_terrain = WildcardRLERoomTerrain::new_from_compressed_terrain(&compressed_terrain);
                        let num_runs = rle_terrain.num_runs();
                        let rle_wildcard_size = rle_terrain.memory_size();
                        rle_wildcard_runs.push(num_runs);

                        if VERBOSE {
                            println!("Wildcard RLE Terrain Size: {}", rle_terrain.memory_size());
                            println!("Num Runs: {}", num_runs);
                        }

                        if compressed_size < rle_packed_size && compressed_size < rle_wildcard_size {
                            rooms_optimal_compressed.push((room_name, compressed_size));
                        } else {
                            if rle_packed_size < rle_wildcard_size {
                                rooms_optimal_rle_packed.push((room_name, rle_packed_size));
                            } else {
                                rooms_optimal_rle_wildcard.push((room_name, rle_wildcard_size));
                            }
                        }
                    }

                    //break; // Only do one room for testing
                }

                let num_rooms_optimal_compressed = rooms_optimal_compressed.len();
                let num_rooms_optimal_rle_packed = rooms_optimal_rle_packed.len();
                let num_rooms_optimal_rle_wildcard = rooms_optimal_rle_wildcard.len();

                rle_packed_runs.sort();
                rle_wildcard_runs.sort();

                let minimum_runs_rle_packed = (&rle_packed_runs).first().copied().unwrap_or(0);
                let minimum_runs_rle_wildcard = (&rle_wildcard_runs).first().copied().unwrap_or(0);

                let compressed_room_terrain_bytes: usize = rooms_optimal_compressed[0].1;

                let needed_compressed_storage: usize = rooms_optimal_compressed.into_iter().map(|(_, s)| s).sum(); 
                let needed_rle_packed_storage: usize = rooms_optimal_rle_packed.into_iter().map(|(_, s)| s).sum(); 
                let needed_rle_wildcard_storage: usize = rooms_optimal_rle_wildcard.into_iter().map(|(_, s)| s).sum();
                let total_storage_needed = needed_compressed_storage + needed_rle_packed_storage + needed_rle_wildcard_storage;
                let compressed_only_total_storage_needed = rooms_processed * compressed_room_terrain_bytes;
                let uncompressed_total_storage_needed = rooms_processed * 2500;

                // Print the calculated stats
                println!("Rooms Processed: {rooms_processed}");
                println!("Rooms optimally stored as compressed: {num_rooms_optimal_compressed}");
                println!("Rooms optimally stored as RLE Packed: {num_rooms_optimal_rle_packed}");
                println!("Rooms optimally stored as RLE Wildcard: {num_rooms_optimal_rle_wildcard}");
                println!("Minimum RLE Packed Runs: {minimum_runs_rle_packed}");
                println!("Minimum RLE Wildcard Runs: {minimum_runs_rle_wildcard}");
                println!("Storage Needed for Compressed Terrain: {needed_compressed_storage}");
                println!("Storage Needed for RLE Packed Terrain: {needed_rle_packed_storage}");
                println!("Storage Needed for RLE Wildcard Terrain: {needed_rle_wildcard_storage}");
                println!("Total Storage Needed (Uncompressed): {uncompressed_total_storage_needed}");
                println!("Total Storage Needed (Compressed Only): {compressed_only_total_storage_needed}");
                println!("Total Storage Needed (Compressed & RLE): {total_storage_needed}");
            }
        }
    }
}



