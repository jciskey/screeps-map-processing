use std::env;
use std::mem::size_of;
use screeps::{RoomName, Terrain};

use screeps_map_processing::compressed_terrain::compressed_terrain::{CompressedRoomTerrain, COMPRESSED_ARRAY_SIZE};
use screeps_map_processing::compressed_terrain_db;
use screeps_map_processing::run_length_encoding::rle_terrain::{RoomTerrainPackedIndexedRLE, BinarySearchPackedRoomTerrainRLE, PackedRLERoomTerrain, RLERoomTerrain, WildcardRLERoomTerrain};
use screeps_map_processing::run_length_encoding::generic_rle::{BinarySearchRLE, IndexedRLE};

pub fn main() {
    let args: Vec<String> = env::args().collect();
    let path_to_compressed_db_file = &args[1];

    println!("== Data Storage Sizes ==");

    // println!("");
    // println!("Bit-packing Terrain Compression:");
    // println!("CompressedRoomTerrain Size: {}", size_of::<CompressedRoomTerrain>());
    // println!("[u8; COMPRESSED_ARRAY_SIZE] Size: {}", size_of::<[u8; COMPRESSED_ARRAY_SIZE]>());

    // println!("");
    // println!("Naive Terrain RLE:");
    // println!("IndexedRLE<Terrain, usize> Size: {}", size_of::<IndexedRLE<Terrain>>());
    // println!("IndexedRLE<Terrain, u32> Size: {}", size_of::<IndexedRLE<Terrain, u32>>());
    // println!("IndexedRLE<Terrain, u16> Size: {}", size_of::<IndexedRLE<Terrain, u16>>());
    // println!("Vec<IndexedRLE<Terrain, usize>> Size: {}", size_of::<Vec<IndexedRLE<Terrain>>>());
    // println!("Vec<IndexedRLE<Terrain, u32>> Size: {}", size_of::<Vec<IndexedRLE<Terrain, u32>>>());
    // println!("Vec<IndexedRLE<Terrain, u16>> Size: {}", size_of::<Vec<IndexedRLE<Terrain, u16>>>());
    // println!("RLERoomTerrain Size: {}", size_of::<RLERoomTerrain>());
    // println!("BinarySearchRLE<Terrain, usize> Size: {}", size_of::<BinarySearchRLE<Terrain>>());
    // println!("BinarySearchRLE<Terrain, u32> Size: {}", size_of::<BinarySearchRLE<Terrain, u32>>());
    // println!("BinarySearchRLE<Terrain, u16> Size: {}", size_of::<BinarySearchRLE<Terrain, u16>>());

    // println!("");
    // println!("Bit-packed Terrain RLE:");
    // println!("RoomTerrainPackedIndexedRLE Size: {}", size_of::<RoomTerrainPackedIndexedRLE>());
    // println!("BinarySearchPackedRoomTerrainRLE Size: {}", size_of::<BinarySearchPackedRoomTerrainRLE>());
    // println!("PackedRLERoomTerrain Size: {}", size_of::<PackedRLERoomTerrain>());

    let rooms_to_check_str = vec!(
        "W23S45", // Very swampy and separated, lots of runs
        "W20S40", // Crossroads, very open, low amount of runs
        "W20S41", // Highway, very open, but does have obstacles, reasonable amount of runs
        "W20S42", // Highway, similar to W20S41
    );

    let rooms_to_check = rooms_to_check_str.iter().filter_map(|name| RoomName::new(name).ok()).collect::<Vec<RoomName>>();

    if let Ok(conn) = compressed_terrain_db::open_db_file(path_to_compressed_db_file) {
        let create_table_res = compressed_terrain_db::create_terrain_table_if_not_exists(&conn);
        if create_table_res.is_ok() {
            let rooms_res = compressed_terrain_db::get_rooms_with_terrain(&conn);
            if let Ok(rooms) = rooms_res {
                for room_name in rooms {
                    if !rooms_to_check.contains(&room_name) {
                        continue;
                    }

                    if let Ok(compressed_terrain) = compressed_terrain_db::get_terrain_for_room(&conn, room_name) {
                        println!("");
                        println!("Room {room_name:?}");

                        println!("CompressedRoomTerrain Size: {}", compressed_terrain.memory_size());

                        // Now that we have the compressed terrain, generate the RLE terrain from
                        // it
                        let rle_terrain = RLERoomTerrain::new_from_compressed_terrain(&compressed_terrain);
                        let num_runs = rle_terrain.num_runs();
                        println!("RLE Terrain u16 Size: {}", rle_terrain.memory_size());
                        println!("Num Runs: {}", num_runs);

                        let rle_terrain = PackedRLERoomTerrain::new_from_compressed_terrain(&compressed_terrain);
                        let num_runs = rle_terrain.num_runs();
                        println!("Bit-packed RLE Terrain Size: {}", rle_terrain.memory_size());
                        println!("Num Runs: {}", num_runs);

                        let rle_terrain = WildcardRLERoomTerrain::new_from_compressed_terrain(&compressed_terrain);
                        let num_runs = rle_terrain.num_runs();
                        println!("Wildcard RLE Terrain Size: {}", rle_terrain.memory_size());
                        println!("Num Runs: {}", num_runs);
                    }

                    //break; // Only do one file for testing
                }
            }
        }
    }
}



